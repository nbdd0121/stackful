use crate::fiber::*;

use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::pin::Pin;

#[cfg(feature = "nightly")]
pub use core::ops::{Generator, GeneratorState};

#[cfg(not(feature = "nightly"))]
pub enum GeneratorState<Y, R> {
    Yielded(Y),
    Complete(R),
}

#[cfg(not(feature = "nightly"))]
pub trait Generator<R = ()> {
    type Yield;
    type Return;

    fn resume(self: Pin<&mut Self>, arg: R) -> GeneratorState<Self::Yield, Self::Return>;
}

pub struct StackfulGenerator<'a, Y, R, Resume> {
    stack: Stack,
    result: Option<StackPointer>,
    func: Option<Box<dyn FnOnce(&YieldHandle<Y, Resume>, Resume) -> R + 'a>>,
    // Make sure this Generator is not Send.
    _marker: PhantomData<*const fn(Resume) -> (Y, R)>,
}

// Everything is movable.
impl<Y, R, Resume> Unpin for StackfulGenerator<'_, Y, R, Resume> {}

pub struct YieldHandle<Y, Resume = ()> {
    stack: Cell<StackPointer>,
    payload: Cell<*mut Y>,
    _marker: PhantomData<Resume>,
}

impl<'a, Y, R, Resume> StackfulGenerator<'a, Y, R, Resume> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(&YieldHandle<Y, Resume>, Resume) -> R + 'a,
    {
        let stack = Stack::allocate();
        Self {
            func: Some(Box::new(f)),
            stack,
            result: None,
            _marker: PhantomData,
        }
    }
}

struct DropPanic;

struct EnterPayload<'a, Y, R, Resume> {
    f: ManuallyDrop<Box<dyn FnOnce(&YieldHandle<Y, Resume>, Resume) -> R + 'a>>,
    p: usize,
}

extern "C" fn enter<Y, R, Resume>(stack: StackPointer, payload: usize) -> FiberReturn {
    let enter = unsafe { &mut *(payload as *mut EnterPayload<'static, Y, R, Resume>) };
    let f = unsafe { ManuallyDrop::take(&mut enter.f) };
    let r = unsafe { (enter.p as *mut Resume).read() };
    let mut yielder = YieldHandle {
        stack: Cell::new(stack),
        payload: Cell::new(enter.p as *mut Y),
        _marker: PhantomData,
    };
    let y = &mut yielder;
    let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || f(y, r)));

    // If the panic is initiated by us, just ignore it. Otherwise it will result in a memory
    // leak.
    if let Err(ref err) = output {
        if err.is::<DropPanic>() {
            return FiberReturn {
                stack: yielder.stack.get(),
                payload: 0,
            };
        }
    }

    unsafe { (yielder.payload.get() as *mut std::thread::Result<R>).write(output) };
    FiberReturn {
        stack: yielder.stack.get(),
        payload: 0,
    }
}

impl<Y, R, Resume> Drop for StackfulGenerator<'_, Y, R, Resume> {
    fn drop(&mut self) {
        if let Some(stack) = self.result {
            unsafe {
                fiber_switch(stack, 0);
            }
        }
    }
}

impl<Y, R, Resume> Generator<Resume> for StackfulGenerator<'_, Y, R, Resume> {
    type Yield = Y;
    type Return = R;

    fn resume(mut self: Pin<&mut Self>, arg: Resume) -> GeneratorState<Y, R> {
        union ResumePayload<Y, R, Resume> {
            resume: ManuallyDrop<Resume>,
            yielded: ManuallyDrop<Y>,
            complete: ManuallyDrop<std::thread::Result<R>>,
        }
        let mut payload = ResumePayload::<Y, R, Resume> {
            resume: ManuallyDrop::new(arg),
        };
        let result = match self.result {
            None => {
                let mut payload = EnterPayload {
                    f: ManuallyDrop::new(self.func.take().expect("polling a completed future")),
                    p: core::ptr::addr_of_mut!(payload) as usize,
                };
                unsafe {
                    fiber_enter(
                        self.stack.top(),
                        core::ptr::addr_of_mut!(payload) as usize,
                        enter::<Y, R, Resume>,
                    )
                }
            }
            Some(v) => {
                unsafe { fiber_switch(v, core::ptr::addr_of_mut!(payload) as usize) }
            }
        };
        self.result = result.stack;

        if result.payload == 0 {
            match unsafe { ManuallyDrop::take(&mut payload.complete) } {
                Err(err) => std::panic::resume_unwind(err),
                Ok(v) => GeneratorState::Complete(v),
            }
        } else {
            GeneratorState::Yielded(unsafe { ManuallyDrop::take(&mut payload.yielded) })
        }
    }
}

impl<Y, Resume> YieldHandle<Y, Resume> {
    pub fn yeet(&self, arg: Y) -> Resume {
        unsafe {
            self.payload.get().write(arg);
            let result = fiber_switch(self.stack.get(), 1);
            self.stack.set(result.stack.unwrap());
            self.payload.set(result.payload as *mut Y);
            if result.payload == 0 {
                std::panic::resume_unwind(Box::new(DropPanic));
            }
            let r = (result.payload as *mut Resume).read();
            r
        }
    }
}

#[test]
fn test_generator() {
    let mut gen = StackfulGenerator::new(|y: &YieldHandle<i32, i32>, mut r: i32| {
        for i in 0..100 {
            assert_eq!(r, i);
            r = y.yeet(i);
        }
        assert_eq!(r, 1024);
        1024
    });
    let mut gen = Pin::new(&mut gen);

    for i in 0..100 {
        assert!(matches!(gen.as_mut().resume(i), GeneratorState::Yielded(x) if x == i));
    }
    assert!(matches!(
        gen.as_mut().resume(1024),
        GeneratorState::Complete(1024)
    ));
}
