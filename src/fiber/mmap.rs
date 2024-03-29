use super::*;
use crate::page_size;

use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Stack(usize);

// Keep a stack so that repeated fiber calls don't require new allocation.
static STACK_CACHE: AtomicUsize = AtomicUsize::new(0);

impl Stack {
    pub fn allocate() -> Self {
        // Before allocating, first check the cache.
        let stack = STACK_CACHE.swap(0, Ordering::Relaxed);
        if stack != 0 {
            return Self(stack);
        }

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
            let ret = libc::mprotect(ptr, page_size, libc::PROT_NONE);
            if ret != 0 {
                panic!("failed to allocated stack");
            }

            Self(ptr as usize)
        }
    }

    #[allow(unused)]
    pub fn bottom(&self) -> usize {
        self.0
    }

    pub fn top(&self) -> StackPointer {
        unsafe { StackPointer(NonZeroUsize::new_unchecked(self.0 + 0x200000)) }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        // Before freeing, first check the cache.
        if STACK_CACHE
            .compare_exchange(0, self.0, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        unsafe { libc::munmap(self.0 as _, 0x200000) };
    }
}
