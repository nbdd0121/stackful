use crate::generator::*;

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

struct Context {
    parent: Cell<Option<&'static Context>>,
    yielder: Cell<Option<&'static YieldHandle<(), &'static Context>>>,
    panicking: Cell<bool>,
    ctx: *mut core::task::Context<'static>,
}

thread_local! {
    static CONTEXT: Cell<Option<&'static Context>> = Cell::new(None);
}

/// Wait for a future to complete and return its output.
///
/// If the function is called directly or recursively from a closure passed to `stackful`,
/// then the `Future` returned by `stackful` would return `Pending`. Otherwise the current
/// thread would block until the future has been completed.
pub fn wait<T>(mut fut: impl Future<Output = T>) -> T {
    let mut context = match CONTEXT.with(|ctx| ctx.get()) {
        Some(v) => v,
        None => {
            // Not called from a fiber context, do a block_on instead.
            return futures_executor::block_on(fut);
        }
    };
    loop {
        // SAFETY: This is safe because we don't move fut.
        if let Poll::Ready(val) = unsafe { Pin::new_unchecked(&mut fut) }
            .as_mut()
            .poll(unsafe { &mut *context.ctx })
        {
            return val;
        }

        CONTEXT.with(|ctx| ctx.set(context.parent.take()));
        let yielder = context.yielder.get().unwrap();

        struct PanicGuard;
        impl Drop for PanicGuard {
            fn drop(&mut self) {
                CONTEXT.with(|ctx| {
                    let context = match ctx.get() {
                        Some(v) => v,
                        None => return,
                    };
                    context.panicking.set(true)
                });
            }
        }

        let guard = PanicGuard;
        context = yielder.yeet(());
        core::mem::forget(guard);

        CONTEXT.with(|ctx| {
            context.parent.set(ctx.take());
            context.yielder.set(Some(yielder));
            ctx.set(Some(context));
        });
    }
}

pub struct StackfulFuture<'a, T> {
    generator: StackfulGenerator<'a, (), T, &'static Context>,
}

impl<'a, T> StackfulFuture<'a, T> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce() -> T + 'a,
    {
        Self {
            generator: StackfulGenerator::new(
                move |y: &YieldHandle<(), &'static Context>, context: &'static Context| {
                    CONTEXT.with(|ctx| {
                        context.parent.set(ctx.take());
                        context.yielder.set(Some(unsafe { std::mem::transmute(y) }));
                        ctx.set(Some(context));
                    });

                    struct ScopeGuard;
                    impl Drop for ScopeGuard {
                        fn drop(&mut self) {
                            CONTEXT.with(|ctx| {
                                let context = match ctx.get() {
                                    Some(v) => v,
                                    None => return,
                                };
                                if context.panicking.get() {
                                    return;
                                }
                                let parent = context.parent.take();
                                ctx.set(parent);
                            });
                        }
                    }

                    let _guard = ScopeGuard;
                    f()
                },
            ),
        }
    }
}

impl<T> Future for StackfulFuture<'_, T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<T> {
        let ctx = Context {
            parent: Cell::new(None),
            yielder: Cell::new(None),
            panicking: Cell::new(false),
            ctx: unsafe { std::mem::transmute(cx) },
        };
        match Pin::new(&mut self.generator).resume(unsafe { std::mem::transmute(&ctx) }) {
            GeneratorState::Yielded(()) => Poll::Pending,
            GeneratorState::Complete(val) => Poll::Ready(val),
        }
    }
}

/// Turn a synchronous function into a `Future`.
///
/// `stackful` can be paired with `wait` to allow async function to be used within a sync function
/// and it can be nested arbitarily deep.
pub async fn stackful<T, F: FnOnce() -> T>(f: F) -> T {
    StackfulFuture::new(f).await
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[should_panic]
fn panick() {
    async_std::task::block_on(stackful(|| {
        wait(async_std::task::yield_now());
        panic!();
    }));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn drop_before_polling() {
    drop(stackful(|| {
        wait(async_std::task::yield_now());
    }));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn drop_after_polling() {
    let waker = futures::task::noop_waker_ref();
    let mut cx = core::task::Context::from_waker(waker);
    let mut fut = Box::pin(stackful(|| {
        wait(async_std::task::yield_now());
    }));
    let _ = Pin::new(&mut fut).poll(&mut cx);
    drop(fut);
    assert!(CONTEXT.with(|ctx| ctx.get()).is_none());
}

#[cfg(not(target_arch = "wasm32"))]
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
