//! Storage implementation which tries to allocate into one storage, and falls back to
//! a second if that fails.
//!
//! Great for small-value optimization, storing inline if an item is small but falling back
//! to the heap for larger values.

#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::ptr;
use core::ptr::{NonNull, Pointee};

use crate::base::{ClonesafeStorage, ExactSizeStorage, LeaksafeStorage, MultiItemStorage, Storage};
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

// SAFETY: Fallback delegates to other impls of storage which must uphold the guarantees
unsafe impl<S1, S2> Storage for FallbackStorage<S1, S2>
where
    S1: Storage,
    S2: Storage,
{
    type Handle<T: ?Sized> = FallbackHandle<S1, S2, T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        match handle {
            // SAFETY: Same safety requirements
            FallbackHandle::First(handle) => unsafe { self.first.get(handle) },
            // SAFETY: Same safety requirements
            FallbackHandle::Second(handle) => unsafe { self.second.get(handle) },
        }
    }

    fn from_raw_parts<T: ?Sized + Pointee>(
        handle: Self::Handle<()>,
        meta: T::Metadata,
    ) -> Self::Handle<T> {
        match handle {
            FallbackHandle::First(handle) => {
                FallbackHandle::First(S1::from_raw_parts(handle, meta))
            }
            FallbackHandle::Second(handle) => {
                FallbackHandle::Second(S2::from_raw_parts(handle, meta))
            }
        }
    }

    fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(S1::cast(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(S2::cast(handle)),
        }
    }

    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(S1::cast_unsized(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(S2::cast_unsized(handle)),
        }
    }

    #[cfg(feature = "unsize")]
    fn coerce<T: ?Sized + Unsize<U>, U: ?Sized>(handle: Self::Handle<T>) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(S1::coerce(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(S2::coerce(handle)),
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
            // SAFETY: Same safety requirements
            FallbackHandle::First(handle) => unsafe { self.first.deallocate_single(handle) },
            // SAFETY: Same safety requirements
            FallbackHandle::Second(handle) => unsafe { self.second.deallocate_single(handle) },
        }
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        match handle {
            FallbackHandle::First(handle) => {
                // SAFETY: Same safety requirements
                let res = unsafe {
                    self.first
                        .try_grow(handle, capacity)
                        .map(FallbackHandle::First)
                };

                if let Ok(handle) = res {
                    return Ok(handle);
                }

                // SAFETY: We require the provided handle is valid
                let old_ptr = unsafe { self.first.get(handle) };
                let old_len = ptr::metadata(old_ptr.as_ptr());

                let new_handle = self.second.allocate_single::<[T]>(capacity)?;
                // SAFETY: We just allocated this handle, it's guaranteed valid
                let new_ptr = unsafe { self.second.get(new_handle).as_ptr().cast::<T>() };

                // SAFETY: Both provided pointers are valid as they're retrieved from valid `get`
                //         calls
                unsafe {
                    ptr::copy::<T>(old_ptr.as_ptr() as *const T, new_ptr, old_len);
                }

                // SAFETY: We require the provided handle is valid, so it's safe to deallocate
                unsafe { self.first.deallocate_single(handle) };

                Ok(FallbackHandle::Second(new_handle))
            }
            // SAFETY: Same safety requirements
            FallbackHandle::Second(handle) => unsafe {
                self.second
                    .try_shrink(handle, capacity)
                    .map(FallbackHandle::Second)
            },
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        match handle {
            // SAFETY: Same safety requirements
            FallbackHandle::First(handle) => unsafe {
                self.first
                    .try_shrink(handle, capacity)
                    .map(FallbackHandle::First)
            },
            // SAFETY: Same safety requirements
            FallbackHandle::Second(handle) => unsafe {
                self.second
                    .try_shrink(handle, capacity)
                    .map(FallbackHandle::Second)
            },
        }
    }
}

// SAFETY: Fallback delegates to other impls of storage which must uphold the guarantees
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
            // SAFETY: Same safety requirements
            FallbackHandle::First(handle) => unsafe { self.first.deallocate(handle) },
            // SAFETY: Same safety requirements
            FallbackHandle::Second(handle) => unsafe { self.second.deallocate(handle) },
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

// SAFETY: Fallback delegates to other impls of storage which must uphold the guarantees
unsafe impl<S1, S2> ClonesafeStorage for FallbackStorage<S1, S2>
where
    S1: ClonesafeStorage,
    S2: ClonesafeStorage,
{
}

// SAFETY: Fallback delegates to other impls of storage which must uphold the guarantees
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

    #[test]
    fn test_try_grow_fallback() {
        let mut f = Store::default();

        let h1 = f.allocate_single::<[u16]>(2).unwrap();
        assert!(matches!(h1, FallbackHandle::First(_)));
        let h2 = unsafe { f.try_grow(h1, 4) }.unwrap();
        assert!(matches!(h2, FallbackHandle::First(_)));
        let h3 = unsafe { f.try_grow(h2, 8) }.unwrap();
        assert!(matches!(h3, FallbackHandle::Second(_)));

        unsafe { f.deallocate_single(h3) };
    }
}
