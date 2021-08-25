use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use super::traits::StaticStorage;

pub struct StorageCell<S>(UnsafeCell<S>, AtomicBool);

impl<S> StorageCell<S> {
    pub const fn new(val: S) -> StorageCell<S> {
        StorageCell(UnsafeCell::new(val), AtomicBool::new(false))
    }

    /// Attempt to claim this StorageCell without locking. Returns
    /// `Some` with the newly created storage if the cell is unclaimed,
    /// otherwise returns `None`.
    pub fn try_claim<T>(&'static self) -> Option<T>
    where
        T: StaticStorage<S>,
    {
        if self.inner_try_claim() {
            Some(T::take_cell(self))
        } else {
            None
        }
    }

    /// Attempt to claim this StorageCell without locking.
    ///
    /// # Panics
    ///
    /// If the StorageCell has already been claimed, either by this or another thread.
    pub fn claim<T>(&'static self) -> T
    where
        T: StaticStorage<S>,
    {
        self.try_claim::<T>()
            .unwrap_or_else(|| panic!("StorageCell already claimed by existing storage"))
    }

    pub(crate) fn release(&self) {
        if !self.inner_try_release() {
            panic!("Couldn't release StorageCell")
        }
    }

    fn inner_try_claim(&self) -> bool {
        let result = self
            .1
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire);

        match result {
            Ok(val) => val == false,
            Err(_) => false,
        }
    }

    fn inner_try_release(&self) -> bool {
        let result = self
            .1
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed);

        result.is_ok()
    }

    pub(super) unsafe fn as_ptr(&self) -> *mut S {
        self.0.get()
    }
}

unsafe impl<S: Send> Send for StorageCell<S> {}
unsafe impl<S: Sync> Sync for StorageCell<S> {}

impl<S> Default for StorageCell<S>
where
    S: Default,
{
    fn default() -> StorageCell<S> {
        StorageCell::new(S::default())
    }
}
