use std::cell::UnsafeCell;
use std::thread::{self, ThreadId};

pub struct SingleThreadCell<T> {
    value: UnsafeCell<T>,
    owner_thread: ThreadId,
}

impl<T> SingleThreadCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            owner_thread: thread::current().id(),
        }
    }

    #[inline]
    fn assert_owner_thread(&self) {
        debug_assert_eq!(
            self.owner_thread,
            thread::current().id(),
            "SingleThreadCell accessed from a non-owner thread"
        );
    }

    #[inline]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.assert_owner_thread();
        let value_ref: &T = unsafe { &*self.value.get() };
        f(value_ref)
    }

    #[inline]
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        self.assert_owner_thread();
        let value_mut: &mut T = unsafe { &mut *self.value.get() };
        f(value_mut)
    }
}

unsafe impl<T> Sync for SingleThreadCell<T> {}
