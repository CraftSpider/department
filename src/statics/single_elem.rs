
use core::ptr::{Pointee, NonNull};
use core::marker::Unsize;
use core::fmt;

use super::StorageCell;
use crate::utils;
use crate::traits::{SingleElementStorage, ElementStorage};

enum SingleElementInner<S: 'static> {
    Mut(*mut S),
    Cell(&'static StorageCell<S>)
}

impl<S: 'static> SingleElementInner<S> {
    fn as_ptr(&self) -> *mut S {
        match self {
            Self::Mut(s) => *s,
            Self::Cell(s) => unsafe { s.get() },
        }
    }
}

pub struct SingleElement<S: 'static>(SingleElementInner<S>);

impl<S: 'static> SingleElement<S> {
    /// Creates a new single-element storage, backed by a mutable static reference.
    pub fn new_mut(storage: &'static mut S) -> SingleElement<S> {
        SingleElement(SingleElementInner::Mut(storage))
    }

    /// Create a new single-element storage, backed by a static reference.
    ///
    /// # Safety
    ///
    /// This type expects unique ownership of the passed reference.
    pub fn new_cell(storage: &'static StorageCell<S>) -> SingleElement<S> {
        storage.claim();
        SingleElement(SingleElementInner::Cell(storage))
    }
}

impl<S> ElementStorage for SingleElement<S> {
    type Handle<T: ?Sized + Pointee> = SingleElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.0.as_ptr()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleElementHandle(meta)
    }
}

impl<S> SingleElementStorage for SingleElement<S> {
    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> crate::traits::Result<Self::Handle<T>> {
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
        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let b = Box::<_, SingleElement<[usize; 4]>>::new_in([1, 2], SingleElement::new_cell(&FOO));
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2]);
    }
}
