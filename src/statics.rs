
use core::cell::UnsafeCell;
use core::ptr;

mod multi_elem;
mod single_elem;

mod multi_range;
mod single_range;

pub use multi_elem::MultiElement;
pub use single_elem::SingleElement;

pub use multi_range::MultiRange;
pub use single_range::SingleRange;

pub struct StorageCell<S>(UnsafeCell<(S, bool)>);

impl<S> StorageCell<S> {
    pub const fn new(val: S) -> StorageCell<S> {
        StorageCell(UnsafeCell::new((val, false)))
    }

    pub unsafe fn get(&self) -> *mut S {
        ptr::addr_of_mut!((*self.0.get()).0)
    }

    fn claim(&self) {
        let bool = unsafe { &mut (*self.0.get()).1 };
        if *bool {
            panic!("StorageCell already claimed by a storage");
        } else {
            *bool = true;
        }
    }
}

unsafe impl<S: Send> Send for StorageCell<S> {}
unsafe impl<S: Sync> Sync for StorageCell<S> {}
