use super::StorageCell;
use crate::base::{MultiRangeStorage, RangeStorage, StorageSafe};
use crate::error::StorageError;
use crate::statics::traits::StaticStorage;
use crate::utils;
use std::marker::PhantomData;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

/// Static multi-range storage implementation
pub struct MultiRange<S: 'static, const N: usize, const M: usize> {
    used: [bool; M],
    storage: &'static StorageCell<[[S; N]; M]>,
}

impl<S: 'static, const N: usize, const M: usize> StaticStorage<[[S; N]; M]>
    for MultiRange<S, N, M>
{
    fn take_cell(storage: &'static StorageCell<[[S; N]; M]>) -> Self {
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
        let idx = std::ptr::addr_of_mut!((*self.storage.as_ptr())[handle.0]);
        let ptr = NonNull::new(idx).expect("Valid handle").cast();
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

pub struct MultiRangeHandle<T>(usize, PhantomData<fn(T) -> T>);

impl<T> Clone for MultiRangeHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for MultiRangeHandle<T> {}
