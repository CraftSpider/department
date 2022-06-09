use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use super::StorageCell;
use crate::base::{RangeStorage, SingleRangeStorage};
use crate::statics::traits::StaticStorage;
use crate::utils;

/// Static single-range storage implementation
pub struct SingleRange<S: 'static, const N: usize>(&'static StorageCell<[S; N]>);

impl<S: 'static, const N: usize> StaticStorage<[S; N]> for SingleRange<S, N> {
    fn take_cell(cell: &'static StorageCell<[S; N]>) -> Self {
        SingleRange(cell)
    }
}

impl<S, const N: usize> RangeStorage for SingleRange<S, N> {
    type Handle<T> = SingleRangeHandle<T>;

    fn maximum_capacity<T>(&self) -> usize {
        (mem::size_of::<S>() * N) / mem::size_of::<T>()
    }

    unsafe fn get<T>(&self, _handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        let ptr = NonNull::new(self.0.as_ptr()).expect("Valid handle").cast();
        NonNull::from_raw_parts(ptr, self.maximum_capacity::<T>())
    }
}

impl<S, const N: usize> SingleRangeStorage for SingleRange<S, N> {
    fn allocate_single<T>(&mut self, capacity: usize) -> crate::error::Result<Self::Handle<T>> {
        utils::validate_array_layout::<T, [MaybeUninit<S>; N]>(capacity)?;
        Ok(SingleRangeHandle(PhantomData))
    }

    unsafe fn deallocate_single<T>(&mut self, _handle: Self::Handle<T>) {}
}

pub struct SingleRangeHandle<T>(PhantomData<fn(T) -> T>);

impl<T> Clone for SingleRangeHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SingleRangeHandle<T> {}
