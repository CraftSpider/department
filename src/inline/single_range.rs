use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::{fmt, mem};

use crate::base::{RangeStorage, SingleRangeStorage, StorageSafe};
use crate::utils;

/// Inline single-range storage implementation
pub struct SingleRange<S, const N: usize> {
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> SingleRange<S, N> {
    /// Create a new `SingleRange`
    pub fn new() -> SingleRange<S, N> {
        SingleRange {
            storage: UnsafeCell::new(MaybeUninit::uninit_array::<N>()),
        }
    }
}

impl<S, const N: usize> RangeStorage for SingleRange<S, N>
where
    S: StorageSafe,
{
    type Handle<T> = SingleRangeHandle<T>;

    fn maximum_capacity<T>(&self) -> usize {
        (mem::size_of::<S>() * N) / mem::size_of::<T>()
    }

    unsafe fn get<T>(&self, _handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        let ptr = NonNull::new(self.storage.get())
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, N)
    }
}

impl<S, const N: usize> SingleRangeStorage for SingleRange<S, N>
where
    S: StorageSafe,
{
    fn allocate_single<T>(&mut self, capacity: usize) -> crate::error::Result<Self::Handle<T>> {
        utils::validate_array_layout::<T, [MaybeUninit<S>; N]>(capacity)?;
        Ok(SingleRangeHandle(PhantomData))
    }

    unsafe fn deallocate_single<T>(&mut self, _handle: Self::Handle<T>) {}
}

impl<S, const N: usize> fmt::Debug for SingleRange<S, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleRange").finish_non_exhaustive()
    }
}

impl<S, const N: usize> Clone for SingleRange<S, N> {
    fn clone(&self) -> Self {
        // 'cloning' doesn't preserve handles, it just gives you a new storage
        SingleRange::new()
    }
}

impl<S, const N: usize> Default for SingleRange<S, N> {
    fn default() -> Self {
        SingleRange::new()
    }
}

pub struct SingleRangeHandle<T>(PhantomData<fn(T) -> T>);

impl<T> Clone for SingleRangeHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SingleRangeHandle<T> {}
