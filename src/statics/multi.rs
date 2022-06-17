use core::alloc::Layout;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::mem;
use core::ptr::{NonNull, Pointee};

use super::StorageCell;
use crate::base::{ExactSizeStorage, MultiItemStorage, Storage, StorageSafe};
use crate::error::{Result, StorageError};
use crate::handles::{Handle, OffsetMetaHandle};
use crate::statics::traits::StaticStorage;
use crate::utils;

/// Static multi-element storage implementation
pub struct MultiStatic<S: 'static, const N: usize> {
    used: [bool; N],
    storage: &'static StorageCell<[S; N]>,
}

impl<S: 'static, const N: usize> StaticStorage<[S; N]> for MultiStatic<S, N> {
    fn take_cell(storage: &'static StorageCell<[S; N]>) -> MultiStatic<S, N> {
        MultiStatic {
            used: [false; N],
            storage,
        }
    }
}

// SAFETY: Internal locks and checks ensure memory safety
unsafe impl<S, const N: usize> Storage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = OffsetMetaHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        // SAFETY: The inner Cell must be claimed as that's the only way to construct a SingleStatic
        let store_ptr = unsafe { self.storage.as_ptr() };
        // SAFETY: The storage pointer is guaranteed valid to dereference
        let idx = unsafe { core::ptr::addr_of_mut!((*store_ptr.as_ptr())[handle.offset()]) };
        let ptr: NonNull<()> = NonNull::new(idx).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.metadata())
    }

    fn from_raw_parts<T: ?Sized + Pointee>(
        handle: Self::Handle<()>,
        meta: T::Metadata,
    ) -> Self::Handle<T> {
        <Self::Handle<T>>::from_raw_parts(handle, meta)
    }

    fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U> {
        handle.cast()
    }

    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        handle.cast_unsized()
    }

    #[cfg(feature = "unsize")]
    fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        handle.coerce()
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: Shares our safety requirements
        unsafe { self.deallocate(handle) }
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.metadata());
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(OffsetMetaHandle::from_offset_meta(
                handle.offset(),
                capacity,
            ))
        } else {
            Err(StorageError::InsufficientSpace {
                expected: new_layout.size(),
                available: Some(self.max_range::<T>()),
            })
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.metadata());
        Ok(OffsetMetaHandle::from_offset_meta(
            handle.offset(),
            capacity,
        ))
    }
}

// SAFETY: Internal locks and checks ensure memory safety
unsafe impl<S, const N: usize> MultiItemStorage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        self.used[pos] = true;

        Ok(OffsetMetaHandle::from_offset_meta(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        self.used[handle.offset()] = false;
    }
}

impl<S, const N: usize> ExactSizeStorage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = Layout::new::<T>();
        mem::size_of::<S>() / layout.size()
    }
}

impl<S, const N: usize> Drop for MultiStatic<S, N> {
    fn drop(&mut self) {
        self.storage.release()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backing::{Align8, Backing};
    use crate::collections::LinkedList;

    #[test]
    fn test_linked_list() {
        static FOO: StorageCell<[Backing<24, Align8>; 4]> = StorageCell::new([Backing::new(); 4]);

        let mut list = LinkedList::<u8, MultiStatic<Backing<24, Align8>, 4>>::new_in(FOO.claim());
        list.push(1);
        list.push(2);

        assert_eq!(list.get(0), Some(&1));
        assert_eq!(list.get(1), Some(&2));
        assert_eq!(list.get(3), None);
    }
}
