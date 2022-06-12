use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem};

use crate::base::{ExactSizeStorage, Storage, StorageSafe};
use crate::error::{Result, StorageError};
use crate::utils;

/// Inline single-element storage implementation
pub struct SingleInline<S> {
    storage: UnsafeCell<MaybeUninit<S>>,
}

impl<S> SingleInline<S> {
    /// Create a new `SingleElement`
    pub fn new() -> SingleInline<S> {
        SingleInline {
            storage: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

unsafe impl<S> Storage for SingleInline<S>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized + Pointee> = SingleInlineHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage.get()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        SingleInlineHandle(handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleInlineHandle(meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;
        Ok(SingleInlineHandle(meta))
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, _handle: Self::Handle<T>) {}

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.0);
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(SingleInlineHandle(capacity))
        } else {
            Err(StorageError::InsufficientSpace(
                new_layout.size(),
                Some(self.max_range::<T>()),
            ))
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.0);
        Ok(SingleInlineHandle(capacity))
    }
}

impl<S> ExactSizeStorage for SingleInline<S>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = utils::layout_of::<T>(());
        mem::size_of::<S>() / layout.size()
    }
}

impl<S> fmt::Debug for SingleInline<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleElement").finish_non_exhaustive()
    }
}

impl<S> Clone for SingleInline<S> {
    fn clone(&self) -> Self {
        // 'cloning' doesn't preserve handles, it just gives you a new storage
        SingleInline::new()
    }
}

impl<S> Default for SingleInline<S> {
    fn default() -> SingleInline<S> {
        SingleInline::new()
    }
}

#[derive(Debug)]
pub struct SingleInlineHandle<T: ?Sized + Pointee>(T::Metadata);

impl<T: ?Sized + Pointee> Clone for SingleInlineHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for SingleInlineHandle<T> {}

#[cfg(test)]
mod tests {
    use crate::boxed::Box;

    use super::*;

    #[test]
    fn test_box() {
        let b = Box::<_, SingleInline<[usize; 4]>>::new([1, 2, 3, 4]);
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_zst() {
        let b = Box::<(), SingleInline<[usize; 0]>>::new(());

        assert_eq!(*b, ());
    }
}
