use core::cell::UnsafeCell;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use super::traits::StaticStorage;

/// A cell to use in statics, allowing them to be 'claimed' by a storage,
/// preventing aliased usage of the backing item.
pub struct StorageCell<S>(UnsafeCell<S>, AtomicBool);

impl<S> StorageCell<S> {
    /// Create a new storage cell containing the provided value
    pub const fn new(val: S) -> StorageCell<S> {
        StorageCell(UnsafeCell::new(val), AtomicBool::new(false))
    }

    /// Attempt to claim this `StorageCell` without locking. Returns
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

    /// Attempt to claim this `StorageCell` without locking.
    ///
    /// # Panics
    ///
    /// If the `StorageCell` has already been claimed, either by this or another thread.
    pub fn claim<T>(&'static self) -> T
    where
        T: StaticStorage<S>,
    {
        self.try_claim::<T>()
            .unwrap_or_else(|| panic!("StorageCell already claimed by existing storage"))
    }

    pub(crate) fn release(&self) {
        assert!(self.inner_try_release(), "Couldn't release StorageCell");
    }

    fn inner_try_claim(&self) -> bool {
        self.1
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
            .map_or(false, |val| !val)
    }

    fn inner_try_release(&self) -> bool {
        self.1
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    pub(super) unsafe fn as_ptr(&self) -> NonNull<S> {
        debug_assert!(
            self.1.load(Ordering::SeqCst),
            "Cell accessed while not claimed"
        );
        // SAFETY: UnsafeCell should never return a null pointer
        unsafe { NonNull::new_unchecked(self.0.get()) }
    }
}

// SAFETY: This type requires as a safety invariant that the inner cell is only accessed while
//         atomically claimed
unsafe impl<S: Send> Send for StorageCell<S> {}
// SAFETY: This type requires as a safety invariant that the inner cell is only accessed while
//         atomically claimed
unsafe impl<S: Sync> Sync for StorageCell<S> {}

impl<S> Default for StorageCell<S>
where
    S: Default,
{
    fn default() -> StorageCell<S> {
        StorageCell::new(S::default())
    }
}
