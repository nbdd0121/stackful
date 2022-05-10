use super::*;

pub struct Stack(usize);

extern "C" {
    fn fiber_create() -> usize;
    fn fiber_destroy(fiber: usize);
}

impl Stack {
    pub fn allocate() -> Self {
        Self(unsafe { fiber_create() })
    }

    pub fn bottom(&self) -> usize {
        // FIXME: Get the actual stack bottom
        0
    }

    pub fn top(&self) -> StackPointer {
        unsafe { StackPointer(NonZeroUsize::new_unchecked(self.0)) }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            fiber_destroy(self.0);
        }
    }
}
