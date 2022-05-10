use core::num::NonZeroUsize;

#[cfg(not(any(target_arch = "wasm32", windows)))]
mod mmap;
#[cfg(not(any(target_arch = "wasm32", windows)))]
pub use mmap::*;

#[cfg(target_arch = "wasm32")]
mod alloc;
#[cfg(target_arch = "wasm32")]
pub use alloc::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct StackPointer(pub NonZeroUsize);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwitchResult {
    pub stack: Option<StackPointer>,
    pub payload: usize,
}

extern "C" {
    pub fn fiber_enter(
        stack: StackPointer,
        payload: usize,
        f: extern "C" fn(StackPointer, payload: usize) -> !,
    ) -> SwitchResult;
    pub fn fiber_switch_enter(stack: StackPointer, payload: usize) -> SwitchResult;
    pub fn fiber_switch_leave(stack: StackPointer, payload: usize) -> SwitchResult;
}
