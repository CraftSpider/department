use core::alloc::Layout;
use core::cell::UnsafeCell;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem};

use crate::base::{ExactSizeStorage, MultiItemStorage, Storage, StorageSafe};
use crate::error::StorageError;
use crate::handles::{Handle, OffsetMetaHandle};
use crate::{error, utils};

/// Inline multi-element storage implementation
pub struct MultiInline<S, const N: usize> {
    used: [bool; N],
    storage: [UnsafeCell<MaybeUninit<S>>; N],
}

impl<S, const N: usize> MultiInline<S, N> {
    /// Create a new `MultiElement`
    pub fn new() -> MultiInline<S, N> {
        MultiInline {
            used: [false; N],
            storage: <[(); N]>::map([(); N], |_| UnsafeCell::new(MaybeUninit::uninit())),
        }
    }
}

// SAFETY: Internal locks and check ensure memory safety
unsafe impl<S, const N: usize> Storage for MultiInline<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized + Pointee> = OffsetMetaHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage[handle.offset()].get())
            .unwrap()
            .cast();
        NonNull::from_raw_parts(ptr, handle.metadata())
    }

    fn from_raw_parts<T: ?Sized + Pointee>(handle: Self::Handle<()>, meta: T::Metadata) -> Self::Handle<T> {
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
    ) -> error::Result<Self::Handle<T>> {
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.deallocate(handle)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
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
    ) -> error::Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.metadata());
        Ok(OffsetMetaHandle::from_offset_meta(
            handle.offset(),
            capacity,
        ))
    }
}

// SAFETY: Internal locks and checks ensure memory safety
unsafe impl<S, const N: usize> MultiItemStorage for MultiInline<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        // Find first unused storage
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

impl<S, const N: usize> ExactSizeStorage for MultiInline<S, N>
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

impl<S, const N: usize> fmt::Debug for MultiInline<S, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiElement").finish_non_exhaustive()
    }
}

impl<S, const N: usize> Clone for MultiInline<S, N> {
    fn clone(&self) -> Self {
        // 'cloning' doesn't preserve handles, it just gives you a new storage
        MultiInline::new()
    }
}

impl<S, const N: usize> Default for MultiInline<S, N> {
    fn default() -> Self {
        MultiInline::new()
    }
}
