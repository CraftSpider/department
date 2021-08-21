use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use crate::utils;
use crate::traits::{RangeStorage, SingleRangeStorage};

pub struct SingleRange<S, const N: usize> {
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> SingleRange<S, N> {
    pub fn new() -> SingleRange<S, N> {
        // SAFETY: This is okay because the whole array is also MaybeUninit
        let storage: [_; N] = unsafe { MaybeUninit::uninit().assume_init() };
        SingleRange {
            storage: UnsafeCell::new(storage),
        }
    }
}

impl<S, const N: usize> RangeStorage for SingleRange<S, N> {
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

impl<S, const N: usize> SingleRangeStorage for SingleRange<S, N> {
    fn allocate_single<T>(&mut self, capacity: usize) -> crate::traits::Result<Self::Handle<T>> {
        utils::validate_array_layout::<T, [MaybeUninit<S>; N]>(capacity)?;
        Ok(SingleRangeHandle(PhantomData))
    }

    unsafe fn deallocate_single<T>(&mut self, _handle: Self::Handle<T>) {}
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
