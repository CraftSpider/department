use core::alloc::Layout;
use core::cell::UnsafeCell;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem};

use crate::base::{ExactSizeStorage, Storage, StorageSafe};
use crate::error::{Result, StorageError};
use crate::handles::MetaHandle;
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

// SAFETY: Internal checks ensure memory safety
unsafe impl<S> Storage for SingleInline<S>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized + Pointee> = MetaHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage.get()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.metadata())
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
        utils::validate_layout::<T, S>(meta)?;
        Ok(MetaHandle::from_metadata(meta))
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, _handle: Self::Handle<T>) {}

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.metadata());
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(MetaHandle::from_metadata(capacity))
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
        Ok(MetaHandle::from_metadata(capacity))
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
        let layout = Layout::new::<T>();
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
    fn test_size() {
        type Box<T> = crate::boxed::Box<T, SingleInline<[u8; 4]>>;

        Box::<[u8; 4]>::try_new([1, 2, 3, 4]).unwrap();
        Box::<[u8; 8]>::try_new([1, 2, 3, 4, 5, 6, 7, 8]).unwrap_err();
    }

    #[test]
    fn test_align() {
        type Box<T, S> = crate::boxed::Box<T, SingleInline<[S; 4]>>;

        #[derive(Debug)]
        #[repr(align(1))]
        struct Align1;
        #[derive(Debug)]
        #[repr(align(2))]
        struct Align2;
        #[derive(Debug)]
        #[repr(align(4))]
        struct Align4;
        #[derive(Debug)]
        #[repr(align(8))]
        struct Align8;

        Box::<_, u8>::try_new(Align1).unwrap();
        Box::<_, u8>::try_new(Align2).unwrap_err();
        Box::<_, u8>::try_new(Align4).unwrap_err();
        Box::<_, u8>::try_new(Align8).unwrap_err();

        Box::<_, u16>::try_new(Align1).unwrap();
        Box::<_, u16>::try_new(Align2).unwrap();
        Box::<_, u16>::try_new(Align4).unwrap_err();
        Box::<_, u16>::try_new(Align8).unwrap_err();

        Box::<_, u32>::try_new(Align1).unwrap();
        Box::<_, u32>::try_new(Align2).unwrap();
        Box::<_, u32>::try_new(Align4).unwrap();
        Box::<_, u32>::try_new(Align8).unwrap_err();

        Box::<_, u64>::try_new(Align1).unwrap();
        Box::<_, u64>::try_new(Align2).unwrap();
        Box::<_, u64>::try_new(Align4).unwrap();
        Box::<_, u64>::try_new(Align8).unwrap();
    }

    #[test]
    fn test_zst() {
        let b = Box::<(), SingleInline<[usize; 0]>>::new(());

        assert_eq!(*b, ());
    }
}
