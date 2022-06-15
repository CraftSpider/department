//! Storage implementation which tries to allocate into one storage, and falls back to
//! a second if that fails.
//!
//! Great for small-value optimization, storing inline if an item is small but falling back
//! to the heap for larger values.

#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};

use crate::base::{ExactSizeStorage, LeaksafeStorage, MultiItemStorage, Storage};
use crate::error;

/// A storage which attempts to store in one storage, then falls back to a second
#[derive(Copy, Clone)]
pub struct FallbackStorage<S1, S2> {
    first: S1,
    second: S2,
}

impl<S1, S2> FallbackStorage<S1, S2> {
    /// Create a new `FallbackStorage` from the two storages to use
    pub fn new(first: S1, second: S2) -> FallbackStorage<S1, S2> {
        FallbackStorage { first, second }
    }

    /// Decompose this storage back into its components
    pub fn decompose(self) -> (S1, S2) {
        (self.first, self.second)
    }
}

impl<S1, S2> Default for FallbackStorage<S1, S2>
where
    S1: Default,
    S2: Default,
{
    fn default() -> Self {
        FallbackStorage {
            first: S1::default(),
            second: S2::default(),
        }
    }
}

unsafe impl<S1, S2> Storage for FallbackStorage<S1, S2>
where
    S1: Storage,
    S2: Storage,
{
    type Handle<T: ?Sized> = FallbackHandle<S1, S2, T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        match handle {
            FallbackHandle::First(handle) => self.first.get(handle),
            FallbackHandle::Second(handle) => self.second.get(handle),
        }
    }

    fn cast<T: ?Sized + Pointee, U>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(self.first.cast(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(self.second.cast(handle)),
        }
    }

    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(self.first.cast_unsized(handle)),
            FallbackHandle::Second(handle) => {
                FallbackHandle::Second(self.second.cast_unsized(handle))
            }
        }
    }

    #[cfg(feature = "unsize")]
    unsafe fn coerce<T: ?Sized + Unsize<U>, U: ?Sized>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(self.first.coerce(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(self.second.coerce(handle)),
        }
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        self.first
            .allocate_single(meta)
            .map(FallbackHandle::First)
            .or_else(|_| {
                self.second
                    .allocate_single(meta)
                    .map(FallbackHandle::Second)
            })
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        match handle {
            FallbackHandle::First(handle) => self.first.deallocate_single(handle),
            FallbackHandle::Second(handle) => self.second.deallocate_single(handle),
        }
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        // TODO: Try to reallocate into second
        match handle {
            FallbackHandle::First(handle) => self
                .first
                .try_grow(handle, capacity)
                .map(FallbackHandle::First),
            FallbackHandle::Second(handle) => self
                .second
                .try_shrink(handle, capacity)
                .map(FallbackHandle::Second),
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        match handle {
            FallbackHandle::First(handle) => self
                .first
                .try_shrink(handle, capacity)
                .map(FallbackHandle::First),
            FallbackHandle::Second(handle) => self
                .second
                .try_shrink(handle, capacity)
                .map(FallbackHandle::Second),
        }
    }
}

unsafe impl<S1, S2> MultiItemStorage for FallbackStorage<S1, S2>
where
    S1: MultiItemStorage,
    S2: MultiItemStorage,
{
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        self.first
            .allocate(meta)
            .map(FallbackHandle::First)
            .or_else(|_| self.second.allocate(meta).map(FallbackHandle::Second))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        match handle {
            FallbackHandle::First(handle) => self.first.deallocate(handle),
            FallbackHandle::Second(handle) => self.second.deallocate(handle),
        }
    }
}

impl<S1, S2> ExactSizeStorage for FallbackStorage<S1, S2>
where
    S1: ExactSizeStorage,
    S2: ExactSizeStorage,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        self.first.will_fit::<T>(meta) || self.second.will_fit::<T>(meta)
    }

    fn max_range<T>(&self) -> usize {
        usize::max(self.first.max_range::<T>(), self.second.max_range::<T>())
    }
}

unsafe impl<S1, S2> LeaksafeStorage for FallbackStorage<S1, S2>
where
    S1: LeaksafeStorage,
    S2: LeaksafeStorage,
{
}

mod private {
    use super::*;

    /// Handle for a fallback storage. Contains either a handle for the first or second storage used
    #[non_exhaustive]
    pub enum FallbackHandle<S1: Storage, S2: Storage, T: ?Sized> {
        /// Allocation uses the first storage
        First(S1::Handle<T>),
        /// Allocation uses the second storage
        Second(S2::Handle<T>),
    }

    impl<S1: Storage, S2: Storage, T: ?Sized> PartialEq for FallbackHandle<S1, S2, T>
    where
        S1::Handle<T>: PartialEq,
        S2::Handle<T>: PartialEq,
    {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (FallbackHandle::First(left), FallbackHandle::First(right)) => left == right,
                (FallbackHandle::Second(left), FallbackHandle::Second(right)) => left == right,
                _ => false,
            }
        }
    }

    impl<S1, S2, T> Clone for FallbackHandle<S1, S2, T>
    where
        S1: Storage,
        S2: Storage,
        T: ?Sized,
    {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl<S1, S2, T> Copy for FallbackHandle<S1, S2, T>
    where
        S1: Storage,
        S2: Storage,
        T: ?Sized,
    {
    }
}

use private::FallbackHandle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::GlobalAlloc;
    use crate::inline::SingleInline;

    type Store = FallbackStorage<SingleInline<[u16; 4]>, GlobalAlloc>;

    #[test]
    fn test_fallback() {
        let mut f = Store::default();

        let h1 = f.create_single::<[u16; 4]>([1, 2, 3, 4]).unwrap();
        assert!(matches!(h1, FallbackHandle::First(_)));
        assert_eq!(unsafe { f.get(h1).as_ref() }, &[1, 2, 3, 4]);

        unsafe { f.drop_single(h1) };

        let h2 = f.create_single::<[u32; 4]>([1, 2, 3, 4]).unwrap();
        assert!(matches!(h2, FallbackHandle::Second(_)));
        assert_eq!(unsafe { f.get(h2).as_ref() }, &[1, 2, 3, 4]);

        unsafe { f.drop_single(h2) };
    }
}
