use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem};

use crate::base::{ExactSizeStorage, MultiItemStorage, Storage, StorageSafe};
use crate::error::StorageError;
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

unsafe impl<S, const N: usize> Storage for MultiInline<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized + Pointee> = MultiInlineHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.storage[handle.0].get()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        MultiInlineHandle(handle.0, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        MultiInlineHandle(handle.0, meta)
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
        debug_assert!(capacity >= handle.1);
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(MultiInlineHandle(handle.0, capacity))
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
    ) -> error::Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.1);
        Ok(MultiInlineHandle(handle.0, capacity))
    }
}

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

        Ok(MultiInlineHandle(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        self.used[handle.0] = false;
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

#[derive(Debug)]
pub struct MultiInlineHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized + Pointee> Clone for MultiInlineHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for MultiInlineHandle<T> {}
