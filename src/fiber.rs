use crate::page_size;
use core::num::NonZeroUsize;
use core::ptr;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct StackPointer(pub NonZeroUsize);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FiberReturn {
    pub stack: StackPointer,
    pub payload: usize,
}

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
        f: extern "C" fn(StackPointer, payload: usize) -> FiberReturn,
    ) -> SwitchResult;
    pub fn fiber_switch(stack: StackPointer, payload: usize) -> SwitchResult;
}

// Layout:
// 0 .. 4096-2*usize: Either F or T, depending on stage
// 4096-2*usize .. 4096-usize: &Waker
// 4096-usize .. 4096: Return stack
pub struct Stack(pub usize);

impl Stack {
    pub fn allocate() -> Self {
        #[cfg(not(target_os = "macos"))]
        use libc::MAP_STACK;
        #[cfg(target_os = "macos")]
        const MAP_STACK: libc::c_int = 0;

        unsafe {
            // Allocate stack
            let ptr = libc::mmap(
                ptr::null_mut(),
                0x200000,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_STACK,
                -1,
                0,
            );
            if ptr.is_null() {
                panic!("failed to allocate stack");
            }

            // Guard page to avoid stack overflow
            let page_size = page_size::get();
            let ret = libc::mprotect(ptr.add(page_size), page_size, libc::PROT_NONE);
            if ret != 0 {
                panic!("failed to allocated stack");
            }

            Self(ptr as usize)
        }
    }

    pub fn bottom(&self) -> usize {
        self.0
    }

    pub fn top(&self) -> usize {
        self.0 + 0x200000
    }

    pub fn as_pointer(&self) -> StackPointer {
        unsafe { StackPointer(NonZeroUsize::new_unchecked(self.top())) }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.0 as _, 0x200000) };
    }
}
