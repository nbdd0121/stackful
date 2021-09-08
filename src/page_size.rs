use std::sync::atomic::{AtomicUsize, Ordering};

static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

pub fn get() -> usize {
    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);
    if ret == 0 {
        ret = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        assert!(ret >= 4096, "page size must be no smaller than 4KiB");
        PAGE_SIZE.store(ret, Ordering::Relaxed);
    }
    ret
}
