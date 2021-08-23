use core::cell::UnsafeCell;
use core::fmt;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};

use crate::traits::{ElementStorage, Result, SingleElementStorage};
use crate::utils;

pub struct SingleElement<S> {
    storage: UnsafeCell<MaybeUninit<S>>,
}

impl<S> SingleElement<S> {
    pub fn new() -> SingleElement<S> {
        SingleElement {
            storage: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl<S> ElementStorage for SingleElement<S> {
    type Handle<T: ?Sized + Pointee> = SingleElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage.get()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleElementHandle(meta)
    }
}

impl<S> SingleElementStorage for SingleElement<S> {
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;
        Ok(SingleElementHandle(meta))
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, _handle: Self::Handle<T>) {}
}

impl<S> fmt::Debug for SingleElement<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleElement").finish_non_exhaustive()
    }
}

impl<S> Clone for SingleElement<S> {
    fn clone(&self) -> Self {
        // 'cloning' doesn't preserve handles, it just gives you a new storage
        SingleElement::new()
    }
}

impl<S> Default for SingleElement<S> {
    fn default() -> SingleElement<S> {
        SingleElement::new()
    }
}

#[derive(Debug)]
pub struct SingleElementHandle<T: ?Sized + Pointee>(T::Metadata);

impl<T: ?Sized + Pointee> Clone for SingleElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for SingleElementHandle<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxed::Box;

    #[test]
    fn test_box() {
        let b = Box::<_, SingleElement<[usize; 4]>>::new([1, 2, 3, 4]);
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2, 3, 4]);
    }
}
