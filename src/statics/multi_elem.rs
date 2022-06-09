use super::StorageCell;
use crate::base::{ElementStorage, MultiElementStorage};
use crate::error::StorageError;
use crate::statics::traits::StaticStorage;
use crate::utils;
use std::marker::Unsize;
use std::ptr::{NonNull, Pointee};

/// Static multi-element storage implementation
pub struct MultiElement<S: 'static, const N: usize> {
    used: [bool; N],
    storage: &'static StorageCell<[S; N]>,
}

impl<S: 'static, const N: usize> StaticStorage<[S; N]> for MultiElement<S, N> {
    fn take_cell(storage: &'static StorageCell<[S; N]>) -> MultiElement<S, N> {
        MultiElement {
            used: [false; N],
            storage,
        }
    }
}

impl<S, const N: usize> ElementStorage for MultiElement<S, N> {
    type Handle<T: ?Sized + Pointee> = MultiElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let idx = std::ptr::addr_of_mut!((*self.storage.as_ptr())[handle.0]);
        let ptr: NonNull<()> = NonNull::new(idx).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        MultiElementHandle(handle.0, meta)
    }
}

impl<S, const N: usize> MultiElementStorage for MultiElement<S, N> {
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> crate::error::Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        Ok(MultiElementHandle(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, _handle: Self::Handle<T>) {}
}

pub struct MultiElementHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized + Pointee> Clone for MultiElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for MultiElementHandle<T> {}
