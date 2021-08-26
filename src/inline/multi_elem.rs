use core::cell::UnsafeCell;
use core::fmt;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};

use crate::error::StorageError;
use crate::base::{ElementStorage, MultiElementStorage, StorageSafe};
use crate::utils;

pub struct MultiElement<S, const N: usize> {
    used: [bool; N],
    storage: [UnsafeCell<MaybeUninit<S>>; N],
}

impl<S, const N: usize> MultiElement<S, N> {
    pub fn new() -> MultiElement<S, N> {
        let mut storage: MaybeUninit<[_; N]> = MaybeUninit::uninit();
        for i in 0..N {
            unsafe { (*storage.as_mut_ptr())[i] = UnsafeCell::new(MaybeUninit::uninit()) };
        }
        let storage = unsafe { storage.assume_init() };

        MultiElement {
            used: [false; N],
            storage,
        }
    }
}

impl<S, const N: usize> ElementStorage for MultiElement<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized + Pointee> = MultiElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage[handle.0].get()).unwrap().cast();
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

impl<S, const N: usize> MultiElementStorage for MultiElement<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> crate::error::Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        // Find first unused storage
        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        self.used[pos] = true;

        Ok(MultiElementHandle(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        self.used[handle.0] = false;
    }
}

impl<S, const N: usize> fmt::Debug for MultiElement<S, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiElement").finish_non_exhaustive()
    }
}

impl<S, const N: usize> Clone for MultiElement<S, N> {
    fn clone(&self) -> Self {
        // 'cloning' doesn't preserve handles, it just gives you a new storage
        MultiElement::new()
    }
}

impl<S, const N: usize> Default for MultiElement<S, N> {
    fn default() -> Self {
        MultiElement::new()
    }
}

#[derive(Debug)]
pub struct MultiElementHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized + Pointee> Clone for MultiElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for MultiElementHandle<T> {}
