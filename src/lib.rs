//! # Bridge between sync and async
//!
//! `stackful` is a minimalistic library that allows you to convert between sync code and async
//! freely.
//!
//! A major issue of Rust's `async` is that it is very intrusive: it pretty much requires a whole
//! program rewrite to convert synchronous code to async, and it would require many libraries to
//! support both variants.
//!
//! Currently, we could use `async_std::task::spawn_blocking` or equivalents to convert blocking
//! IO into async IO using thread pools, or use `async_std::task::block_on` to use an async library
//! in blocking code. But it is still quite problematic if you want to use a synchronous middleware
//! inside an generally asynchronous program.
//!
//! `stackful` aims to blur the boundary by using stackful coroutines to allow arbitary yield
//! points within other-wise synchronous code.
//!
//! ## Usage
//! Use `wait` to convert an async value to a sync value, similar to `.await` or `block_on`.
//!
//! ```
//! # use std::time::Duration;
//! use stackful::wait;
//! # async_std::task::block_on(stackful::stackful(|| {
//! wait(async_std::task::sleep(Duration::from_secs(1)));
//! # }));
//! ```
//!
//! Use `stackful` to convert a synchronous function into a `Future`:
//! ```
//! use stackful::stackful;
//! # async_std::task::spawn(async {
//! async_std::task::spawn_local(stackful(|| {
//!     // Synchronous code
//!     // This shouldn't block, however
//! }));
//! # });
//! ```
//! You can combine these functions, note that we seamlessly handle nested functions:
//! ```
//! # use std::time::Duration;
//! use stackful::{stackful, wait};
//!
//! fn maybe_sleep(dur: Option<Duration>) {
//!     // This is not possible under `async`!
//!     dur.map(|x| wait(async_std::task::sleep(x)));
//! }
//!
//! # async_std::task::spawn(async {
//! async_std::task::spawn_local(async {
//!     stackful(|| maybe_sleep(Some(Duration::from_secs(1)))).await
//! });
//! # });
//! ```

use std::cell::Cell;
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll, Waker};
use core::num::NonZeroUsize;

mod page_size;
mod fiber;
use fiber::*;

thread_local! {
    static STACK: Cell<usize> = Cell::new(0);
}

const OFFSET_WAKER: usize = 4096 - 2 * mem::size_of::<usize>();
const OFFSET_RETURN: usize = 4096 - mem::size_of::<usize>();

// Yield control from the current fiber
fn fiber_yield() {
    let stack_bottom = STACK.with(|cell| cell.get());
    debug_assert_ne!(stack_bottom, 0);

    unsafe {
        let stack = ptr::read((stack_bottom + OFFSET_RETURN) as *const usize);
        let result = fiber_switch(StackPointer(NonZeroUsize::new_unchecked(stack)), 0);
        debug_assert!(result.stack.is_some());
        ptr::write((stack_bottom + OFFSET_RETURN) as *mut usize, result.stack.unwrap_unchecked().0.get());
    }
}

struct DropPanic;

/// Wait for a future to complete and return its output.
///
/// If the function is called directly or recursively from a closure passed to `stackful`,
/// then the `Future` returned by `stackful` would return `Pending`. Otherwise the current
/// thread would block until the future has been completed.
pub fn wait<T>(mut fut: impl Future<Output = T>) -> T {
    let stack_bottom = STACK.with(|cell| cell.get());

    // Not called from a fiber context, do a block_on instead.
    if stack_bottom == 0 {
        return futures_executor::block_on(fut);
    }

    loop {
        // Get the context to use
        let waker_ref = match unsafe {
            ptr::read((stack_bottom + OFFSET_WAKER) as *const Option<&'static Waker>)
        } {
            None => std::panic::resume_unwind(Box::new(DropPanic)),
            Some(v) => v,
        };
        let mut context = Context::from_waker(waker_ref);

        // SAFETY: This is safe because we don't move fut.
        if let Poll::Ready(val) = unsafe { Pin::new_unchecked(&mut fut) }
            .as_mut()
            .poll(&mut context)
        {
            return val;
        }
        fiber_yield();
    }
}

struct Stackful {
    stack: Stack,
    result: Option<SwitchResult>,
}

impl Drop for Stackful {
    fn drop(&mut self) {
        match self.result {
            Some(SwitchResult { stack: Some(_), .. }) => {
                self.abort();
            }
            _ => (),
        }
    }
}

impl Stackful {
    // Abort execution half-way.
    #[cold]
    fn abort(&mut self) {
        // We need to use the guard to make sure lifetime is correct in case of panic.
        struct Guard(usize);
        impl Drop for Guard {
            fn drop(&mut self) {
                STACK.with(|cell| cell.set(self.0));
            }
        }
        let _guard = STACK.with(|cell| {
            let bottom = self.stack.bottom();
            // Update the waker reference to None so `wait` will panic.
            unsafe {
                ptr::write((bottom + OFFSET_WAKER) as *mut Option<&Waker>, None);
            }
            let bottom = cell.replace(bottom);
            Guard(bottom)
        });

        let result = unsafe { fiber_switch(self.result.unwrap().stack.unwrap_unchecked(), 0) };
        assert!(result.stack.is_none());
    }

    fn poll(&mut self, cx: &mut Context<'_>, f: extern "C" fn(StackPointer, usize) -> FiberReturn) -> Poll<()> {
        // We need to use the guard to make sure lifetime is correct in case of panic.
        struct Guard(usize);
        impl Drop for Guard {
            fn drop(&mut self) {
                STACK.with(|cell| cell.set(self.0));
            }
        }
        let _guard = STACK.with(|cell| {
            let bottom = self.stack.bottom();
            // Update the waker reference.
            unsafe {
                ptr::write(
                    (bottom + OFFSET_WAKER) as *mut Option<&Waker>,
                    Some(cx.waker()),
                );
            }
            let bottom = cell.replace(bottom);
            Guard(bottom)
        });

        let result = match self.result {
            None => unsafe { fiber_enter(self.stack.as_pointer(), 0, f) },
            Some(v) => {
                assert!(v.stack.is_some(), "polling a completed future");
                unsafe { fiber_switch(v.stack.unwrap_unchecked(), 0) }
            }
        };

        self.result = Some(result);
        if result.stack.is_none() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

struct StackfulFuture<T, F> {
    inner: Stackful,
    // Make sure this Future is not Send.
    _marker: PhantomData<*mut (T, F)>,
}

impl<T, F: FnOnce() -> T> StackfulFuture<T, F> {
    extern "C" fn enter(stack: StackPointer, payload: usize) -> FiberReturn {
        let stack = stack.0.get();
        let stack_bottom = STACK.with(|cell| cell.get());

        // Save the return stack pointer here.
        unsafe {
            ptr::write((stack_bottom + OFFSET_RETURN) as *mut usize, stack);
        }

        // SAFETY: enter is called only once, so at this time is F is available at bottom of the stack.
        let f = unsafe { ptr::read(stack_bottom as *const F) };

        let output = panic::catch_unwind(AssertUnwindSafe(f));

        // Retrieve the return stack pointer here.
        let stack = unsafe { ptr::read((stack_bottom + OFFSET_RETURN) as *const usize) };

        // If the panic is initiated by us, just ignore it. Otherwise it will result in a memory
        // leak.
        if let Err(ref err) = output {
            if err.is::<DropPanic>() {
                return FiberReturn {
                    stack: unsafe{StackPointer(NonZeroUsize::new_unchecked(stack))},
                    payload: 0,
                };
            }
        }

        // SAFETY: we checked that the size and alignment is okay when constructing.
        unsafe { ptr::write(stack_bottom as *mut std::thread::Result<T>, output) };

        FiberReturn {
            stack: unsafe{StackPointer(NonZeroUsize::new_unchecked(stack))},
            payload: 0,
        }
    }
}

impl<T, F: FnOnce() -> T> Future for StackfulFuture<T, F> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.inner.poll(cx, Self::enter).map(|_| {
            match unsafe { ptr::read(self.inner.stack.bottom() as *const std::thread::Result<T>) } {
                Ok(v) => v,
                Err(v) => panic::resume_unwind(v),
            }
        })
    }
}

/// Turn a synchronous function into a `Future`.
///
/// `stackful` can be paired with `wait` to allow async function to be used within a sync function
/// and it can be nested arbitarily deep.
pub async fn stackful<T, F: FnOnce() -> T>(f: F) -> T {
    let stack = Stack::allocate();

    // For convience we use the bottom of the stack to pass data structure around. Check that types
    // doesn't violate the constraints.
    assert!(mem::size_of::<std::thread::Result<T>>() <= OFFSET_WAKER);
    assert!(mem::align_of::<std::thread::Result<T>>() <= 4096);
    assert!(mem::size_of::<F>() <= OFFSET_WAKER);
    assert!(mem::align_of::<F>() <= 4096);

    // SAFETY: We just checked it's okay.
    unsafe { ptr::write(stack.bottom() as *mut F, f) };

    StackfulFuture::<T, F> {
        inner: Stackful {
            stack,
            result: None,
        },
        _marker: PhantomData,
    }
    .await
}

#[test]
#[should_panic]
fn panick() {
    async_std::task::block_on(stackful(|| {
        panic!();
    }));
}

#[test]
fn drop_before_polling() {
    drop(stackful(|| {
        wait(async_std::task::yield_now());
    }));
}

#[test]
fn drop_after_polling() {
    let waker = futures::task::noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    let mut fut = Box::pin(stackful(|| {
        wait(async_std::task::yield_now());
    }));
    let _ = Pin::new(&mut fut).poll(&mut cx);
}

#[test]
fn test() {
    async_std::task::block_on(stackful(|| {
        eprintln!("A");
        wait(async_std::task::yield_now());
        eprintln!("B");
        wait(async_std::task::sleep(std::time::Duration::from_secs(1)));
        eprintln!("C");
    }));
    wait(async_std::task::yield_now());
    eprintln!("D");
}
