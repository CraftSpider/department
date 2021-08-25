use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use crate::error::StorageError;
use crate::traits::{MultiRangeStorage, RangeStorage};
use crate::utils;

pub struct MultiRange<S, const N: usize, const M: usize> {
    used: [bool; M],
    storage: [UnsafeCell<[MaybeUninit<S>; N]>; M],
}

impl<S, const N: usize, const M: usize> MultiRange<S, N, M> {
    pub fn new() -> MultiRange<S, N, M> {
        let mut storage: MaybeUninit<[_; M]> = MaybeUninit::uninit();
        for i in 0..M {
            unsafe {
                (*storage.as_mut_ptr())[i] = UnsafeCell::new(MaybeUninit::uninit_array::<N>())
            };
        }
        let storage = unsafe { storage.assume_init() };

        MultiRange {
            used: [false; M],
            storage,
        }
    }
}

impl<S, const N: usize, const M: usize> RangeStorage for MultiRange<S, N, M> {
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

impl<S, const N: usize, const M: usize> MultiRangeStorage for MultiRange<S, N, M> {
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
