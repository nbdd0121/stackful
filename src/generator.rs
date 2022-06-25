use crate::fiber::*;

use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::pin::Pin;

#[cfg(feature = "nightly")]
pub use core::ops::{Generator, GeneratorState};

#[cfg(not(feature = "nightly"))]
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
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
    #[cfg(feature = "stacker")]
    stack_limit: Option<usize>,
    func: Option<Box<dyn FnOnce(&YieldHandle<Y, Resume>, Resume) -> R + 'a>>,
    // Make sure this Generator is not Send.
    _marker: PhantomData<*const fn(Resume) -> (Y, R)>,
}

// Everything is movable.
impl<Y, R, Resume> Unpin for StackfulGenerator<'_, Y, R, Resume> {}

pub struct YieldHandle<Y, Resume = ()> {
    stack: Cell<StackPointer>,
    _marker: PhantomData<(Y, Resume)>,
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
            #[cfg(feature = "stacker")]
            stack_limit: None,
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

enum YieldPayload {
    Yielded(*const ()),
    Complete(*const ()),
    Panic(*mut (dyn std::any::Any + Send)),
}

extern "C" fn enter<Y, R, Resume>(stack: StackPointer, payload: usize) -> ! {
    let enter = unsafe { &mut *(payload as *mut EnterPayload<'static, Y, R, Resume>) };
    let f = unsafe { ManuallyDrop::take(&mut enter.f) };
    let r = unsafe { (enter.p as *mut Resume).read() };
    let mut yielder = YieldHandle {
        stack: Cell::new(stack),
        _marker: PhantomData,
    };
    let y = &mut yielder;
    let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || f(y, r)));

    let payload = match output {
        Ok(ref output) => YieldPayload::Complete(output as *const _ as _),
        Err(err) => YieldPayload::Panic(Box::into_raw(err)),
    };
    unsafe {
        fiber_switch_leave(yielder.stack.get(), &payload as *const _ as _);
    }

    unreachable!("resuming a completed generator");
}

impl<Y, R, Resume> Drop for StackfulGenerator<'_, Y, R, Resume> {
    fn drop(&mut self) {
        if let Some(stack) = self.result {
            // This will give us a `YieldPayload::Panic(DropPanic)`, but we can safely ignore it
            // because DropPanic is a ZST.
            unsafe {
                fiber_switch_enter(stack, 0);
            }
        }
    }
}

impl<Y, R, Resume> Generator<Resume> for StackfulGenerator<'_, Y, R, Resume> {
    type Yield = Y;
    type Return = R;

    fn resume(mut self: Pin<&mut Self>, arg: Resume) -> GeneratorState<Y, R> {
        let payload = &arg as *const _ as usize;
        #[cfg(feature = "stacker")]
        let stack_limit = stacker::get_stack_limit();
        let result = match self.result {
            None => {
                let mut payload = EnterPayload {
                    f: ManuallyDrop::new(self.func.take().expect("polling a completed future")),
                    p: payload,
                };
                #[cfg(feature = "stacker")]
                stacker::set_stack_limit(Some(self.stack.bottom()));
                unsafe {
                    fiber_enter(
                        self.stack.top(),
                        core::ptr::addr_of_mut!(payload) as usize,
                        enter::<Y, R, Resume>,
                    )
                }
            }
            Some(v) => {
                #[cfg(feature = "stacker")]
                stacker::set_stack_limit(self.stack_limit);
                unsafe { fiber_switch_enter(v, payload) }
            }
        };
        std::mem::forget(arg);
        self.result = result.stack;
        #[cfg(feature = "stacker")]
        {
            self.stack_limit = stacker::get_stack_limit();
            stacker::set_stack_limit(stack_limit);
        }

        let y_payload = unsafe { (result.payload as *const YieldPayload).read() };

        match y_payload {
            YieldPayload::Yielded(y) => GeneratorState::Yielded(unsafe { (y as *const Y).read() }),
            YieldPayload::Complete(r) => {
                self.result = None;
                GeneratorState::Complete(unsafe { (r as *const R).read() })
            }
            YieldPayload::Panic(p) => {
                self.result = None;
                std::panic::resume_unwind(unsafe { Box::from_raw(p) });
            }
        }
    }
}

impl<Y, Resume> YieldHandle<Y, Resume> {
    pub fn yeet(&self, arg: Y) -> Resume {
        unsafe {
            // `arg` is passed by reference. It lives on the stack of the current fiber, which
            // will be valid while the fiber hile the current fiber is suspended. `forget` it
            // after `fiber_switch` because the ownership is transferred to the target fiber.
            let payload = YieldPayload::Yielded(&arg as *const Y as _);
            let result = fiber_switch_leave(self.stack.get(), &payload as *const YieldPayload as _);
            std::mem::forget(arg);

            self.stack.set(result.stack.unwrap());
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
