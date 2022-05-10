use super::*;

pub struct Stack(pub usize);

impl Stack {
    pub fn allocate() -> Self {
        Self(unsafe {
            std::alloc::alloc(std::alloc::Layout::from_size_align(0x200000, 16).unwrap()) as usize
        })
    }

    pub fn bottom(&self) -> usize {
        self.0
    }

    pub fn top(&self) -> StackPointer {
        unsafe { StackPointer(NonZeroUsize::new_unchecked(self.0 + 0x200000)) }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(
                self.0 as *mut u8,
                std::alloc::Layout::from_size_align(0x200000, 16).unwrap(),
            );
        }
    }
}
