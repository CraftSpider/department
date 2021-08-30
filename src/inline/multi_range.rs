use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use crate::base::{MultiRangeStorage, RangeStorage, StorageSafe};
use crate::error::StorageError;
use crate::utils;

/// Inline multi-range storage implementation
pub struct MultiRange<S, const N: usize, const M: usize> {
    used: [bool; M],
    storage: [UnsafeCell<[MaybeUninit<S>; N]>; M],
}

impl<S, const N: usize, const M: usize> MultiRange<S, N, M> {
    /// Create a new `MultiRange`
    pub fn new() -> MultiRange<S, N, M> {
        let storage: [_; M] = <[(); M]>::map([(); M], |_| {
            UnsafeCell::new(<[(); N]>::map([(); N], |_| MaybeUninit::uninit()))
        });

        MultiRange {
            used: [false; M],
            storage,
        }
    }
}

impl<S, const N: usize, const M: usize> RangeStorage for MultiRange<S, N, M>
where
    S: StorageSafe,
{
    type Handle<T> = MultiRangeHandle<T>;

    fn maximum_capacity<T>(&self) -> usize {
        (mem::size_of::<S>() * N) / mem::size_of::<T>()
    }

    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        let ptr = NonNull::new(self.storage[handle.0].get())
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, N)
    }
}

impl<S, const N: usize, const M: usize> MultiRangeStorage for MultiRange<S, N, M>
where
    S: StorageSafe,
{
    fn allocate<T>(&mut self, capacity: usize) -> crate::error::Result<Self::Handle<T>> {
        utils::validate_array_layout::<T, [MaybeUninit<S>; N]>(capacity)?;

        // Find first unused storage
        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        self.used[pos] = true;

        Ok(MultiRangeHandle(pos, PhantomData))
    }

    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>) {
        self.used[handle.0] = false;
    }
}

impl<S, const N: usize, const M: usize> Default for MultiRange<S, N, M> {
    fn default() -> Self {
        MultiRange::new()
    }
}

pub struct MultiRangeHandle<T>(usize, PhantomData<fn(T) -> T>);

impl<T> Clone for MultiRangeHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for MultiRangeHandle<T> {}
