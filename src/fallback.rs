//! Storage implementation which tries to allocate into one storage, and falls back to
//! a second if that fails.
//!
//! Great for small-value optimization, storing inline if an item is small but falling back
//! to the heap for larger values.

use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};

use crate::base::{ExactSizeStorage, LeaksafeStorage, MultiItemStorage, Storage};
use crate::error;

/// A storage which attempts to store in one storage, then falls back to a second
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
    type Handle<T: ?Sized> = FallbackHandle<S1::Handle<T>, S2::Handle<T>>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        match handle {
            FallbackHandle::First(handle) => self.first.get(handle),
            FallbackHandle::Second(handle) => self.second.get(handle),
        }
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(self.first.cast(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(self.second.cast(handle)),
        }
    }

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

/*impl<S1, S2> ElementStorage for FallbackStorage<S1, S2>
where
    S1: ElementStorage,
    S2: ElementStorage,
{
    type Handle<T: ?Sized + Pointee> = FallbackHandle<S1::Handle<T>, S2::Handle<T>>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        match handle {
            FallbackHandle::First(handle) => self.first.get(handle),
            FallbackHandle::Second(handle) => self.second.get(handle),
        }
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        match handle {
            FallbackHandle::First(handle) => FallbackHandle::First(self.first.coerce(handle)),
            FallbackHandle::Second(handle) => FallbackHandle::Second(self.second.coerce(handle)),
        }
    }
}

impl<S1, S2> SingleElementStorage for FallbackStorage<S1, S2>
where
    S1: SingleElementStorage,
    S2: SingleElementStorage,
{
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

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        match handle {
            FallbackHandle::First(handle) => self.first.deallocate_single(handle),
            FallbackHandle::Second(handle) => self.second.deallocate_single(handle),
        }
    }
}

impl<S1, S2> MultiElementStorage for FallbackStorage<S1, S2>
where
    S1: MultiElementStorage,
    S2: MultiElementStorage,
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

impl<S1, S2> RangeStorage for FallbackStorage<S1, S2>
where
    S1: RangeStorage,
    S2: RangeStorage,
{
    type Handle<T> = FallbackHandle<S1::Handle<T>, S2::Handle<T>>;

    fn maximum_capacity<T>(&self) -> usize {
        usize::max(
            self.first.maximum_capacity::<T>(),
            self.second.maximum_capacity::<T>(),
        )
    }

    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        match handle {
            FallbackHandle::First(handle) => self.first.get(handle),
            FallbackHandle::Second(handle) => self.second.get(handle),
        }
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
        // TODO: Try to reallocate into second
        match handle {
            FallbackHandle::First(handle) => self
                .first
                .try_grow(handle, capacity)
                .map(FallbackHandle::First),
            FallbackHandle::Second(handle) => self
                .second
                .try_grow(handle, capacity)
                .map(FallbackHandle::Second),
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
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

impl<S1, S2> SingleRangeStorage for FallbackStorage<S1, S2>
where
    S1: SingleRangeStorage,
    S2: SingleRangeStorage,
{
    fn allocate_single<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>> {
        self.first
            .allocate_single(capacity)
            .map(FallbackHandle::First)
            .or_else(|_| {
                self.second
                    .allocate_single(capacity)
                    .map(FallbackHandle::Second)
            })
    }

    unsafe fn deallocate_single<T>(&mut self, handle: Self::Handle<T>) {
        match handle {
            FallbackHandle::First(handle) => self.first.deallocate_single(handle),
            FallbackHandle::Second(handle) => self.second.deallocate_single(handle),
        }
    }
}

impl<S1, S2> MultiRangeStorage for FallbackStorage<S1, S2>
where
    S1: MultiRangeStorage,
    S2: MultiRangeStorage,
{
    fn allocate<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>> {
        self.first
            .allocate(capacity)
            .map(FallbackHandle::First)
            .or_else(|_| self.second.allocate(capacity).map(FallbackHandle::Second))
    }

    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>) {
        match handle {
            FallbackHandle::First(handle) => self.first.deallocate(handle),
            FallbackHandle::Second(handle) => self.second.deallocate(handle),
        }
    }
}

// SAFETY: We can leak this storage if we can leak either of its components
unsafe impl<S1, S2> LeaksafeStorage for FallbackStorage<S1, S2>
where
    S1: LeaksafeStorage,
    S2: LeaksafeStorage,
{
}*/

/// Handle for a fallback storage. Contains either a handle for the first or second storage used
#[derive(Copy, Clone)]
#[non_exhaustive]
pub enum FallbackHandle<H1, H2> {
    /// Allocation uses the first storage
    First(H1),
    /// Allocation uses the second storage
    Second(H2),
}

// TODO: Tests
