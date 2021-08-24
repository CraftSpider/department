use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{Pointee, NonNull};
use core::{mem, ptr};
use core::cell::UnsafeCell;
use core::alloc::Layout;

use crate::traits::{ElementStorage, SingleElementStorage, Result};
use crate::utils;
use crate::error::StorageError;

fn blocks<S>(size: usize) -> usize {
    (size + 3) / mem::size_of::<S>()
}

pub struct StaticHeap<S, const N: usize> {
    used: spin::Mutex<[bool; N]>,
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> StaticHeap<S, N> {
    pub const fn new() -> StaticHeap<S, N> {
        StaticHeap {
            used: spin::Mutex::new([false; N]),
            storage: UnsafeCell::new(MaybeUninit::uninit_array::<N>())
        }
    }

    fn try_lock(&self, size: usize) -> Result<usize> {
        let start = self.find_open(size)?;

        for i in &mut self.used.lock()[start..(start + blocks::<S>(size))] {
            *i = true;
        }

        Ok(start)
    }

    fn unlock(&self, start: usize, size: usize) {
        for i in &mut self.used.lock()[start..(start + blocks::<S>(size))] {
            *i = false
        }
    }

    fn find_open(&self, size: usize) -> Result<usize> {
        if blocks::<S>(size) > N {
            return Err(StorageError::InsufficientSpace(size, Some(mem::size_of::<S>() * N)));
        }

        self.used
            .lock()
            .iter()
            .scan(0, |n, &v| {
                if !v {
                    *n += 1
                } else {
                    *n = 0
                }
                Some(*n)
            })
            .position(|n| n == blocks::<S>(size))
            .map(|n| (n - (blocks::<S>(size) - 1)))
            .ok_or(StorageError::NoSlots)
    }
}

impl<S, const N: usize> ElementStorage for &StaticHeap<S, N> {
    type Handle<T: ?Sized + Pointee> = HeapElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr = NonNull::new(ptr::addr_of_mut!((*self.storage.get())[handle.0]))
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        HeapElementHandle(handle.0, meta)
    }
}

impl<S, const N: usize> SingleElementStorage for &StaticHeap<S, N> {
    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);
        utils::validate_layout_for::<[S; N]>(layout)?;
        let start = self.try_lock(layout.size())?;
        Ok(HeapElementHandle(start, meta))
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(self.get(handle).as_ref());
        self.unlock(handle.0, layout.size());
    }
}

unsafe impl<S: Send, const N: usize> Send for StaticHeap<S, N> {}
unsafe impl<S: Sync, const N: usize> Sync for StaticHeap<S, N> {}

pub struct HeapElementHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized> Clone for HeapElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for HeapElementHandle<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxed::Box;

    #[test]
    fn test_box() {
        static HEAP: StaticHeap<usize, 4> = StaticHeap::new();
        let b = Box::new_in([1, 2], &HEAP);
        let b2 = b.coerce::<[i32]>();

        assert_eq!(&*b2, &[1, 2]);
    }

    #[test]
    fn test_multi_box() {
        static HEAP: StaticHeap<usize, 16> = StaticHeap::new();
        let b1 = Box::new_in([1, 2], &HEAP);
        let b2 = Box::new_in([3, 4], &HEAP);
        let b3 = Box::new_in([5, 6], &HEAP);
        let b4 = Box::new_in([7, 8], &HEAP);

        assert_eq!(*b1, [1, 2]);
        assert_eq!(*b2, [3, 4]);
        assert_eq!(*b3, [5, 6]);
        assert_eq!(*b4, [7, 8]);
    }
}
