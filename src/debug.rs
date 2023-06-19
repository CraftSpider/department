//! Storage implementation which wraps another storage implementation, and provides runtime panics
//! for most forms of incorrect usage.
//!
//! This will not catch *all* UB, but it should catch most obviously incorrect usages.

#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};
use spin::Mutex;

use crate::alloc::GlobalAlloc;
use crate::base::{ExactSizeStorage, LeaksafeStorage, MultiItemStorage, Storage};
use crate::collections::Vec;
use crate::handles::Handle;

struct DebugState<S: Storage> {
    single_allocated: Option<DebugHandle<S, ()>>,
    id: usize,
    allocated_handles: Vec<DebugHandle<S, ()>, GlobalAlloc>,
    deallocated_handles: Vec<DebugHandle<S, ()>, GlobalAlloc>,
}

impl<S: Storage> DebugState<S> {
    fn new() -> DebugState<S> {
        DebugState {
            single_allocated: None,
            id: 0,
            allocated_handles: Vec::new(),
            deallocated_handles: Vec::new(),
        }
    }
}

/// A storage which provides runtime panics for many forms of storage UB
pub struct Debug<S: Storage>(Mutex<DebugState<S>>, S);

impl<S> Debug<S>
where
    S: Storage,
{
    /// Create a new [`Debug`][struct@Debug] from an existing storage
    pub fn new(storage: S) -> Debug<S> {
        Debug(Mutex::new(DebugState::new()), storage)
    }

    fn validate_get(&self, handle: DebugHandle<S, ()>) {
        let lock = self.0.lock();

        if let Some(alloc_handle) = lock.single_allocated {
            assert_eq!(alloc_handle, handle, "Attempted to access single allocation with incorrect handle");
        }

        assert!(
            !lock.deallocated_handles.contains(&handle),
            "Attempting to access allocation with deallocated handle",
        );
        assert!(
            lock.allocated_handles.contains(&handle),
            "Attempting to access allocation with never-allocated handle"
        );
    }

    fn validate_alloc(&self, single: bool, handle: S::Handle<()>) -> usize {
        let mut lock = self.0.lock();

        let id = lock.id;
        lock.id += 1;

        let handle = DebugHandle { id, handle };

        if single {
            assert!(lock.single_allocated.is_none(), "Called allocate_single without calling deallocate_single - this may overwrite the old value");
            lock.single_allocated = Some(handle);
        }

        lock.allocated_handles.push(handle);

        id
    }

    fn validate_dealloc(&self, single: bool, handle: DebugHandle<S, ()>) {
        let mut lock = self.0.lock();

        assert!(!lock.deallocated_handles.contains(&handle), "Called deallocate_single on the same handle twice");

        if single {
            assert!(lock.single_allocated.is_some(), "Called deallocate_single without first allocating");
            lock.single_allocated = None;
        }

        lock.allocated_handles
            .iter()
            .position(|h| *h == handle)
            .map(|pos| lock.allocated_handles.remove(pos));

        lock.deallocated_handles.push(handle);
    }
}

// SAFETY: Debug delegates to another implementor of `Storage` which must uphold the guarantees
unsafe impl<S> Storage for Debug<S>
where
    S: Storage,
{
    type Handle<T: ?Sized> = DebugHandle<S, T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        self.validate_get(Self::cast(handle));
        // SAFETY: Shares our safety requirements
        unsafe { self.1.get::<T>(handle.handle) }
    }

    fn from_raw_parts<T: ?Sized + Pointee>(
        handle: Self::Handle<()>,
        meta: T::Metadata,
    ) -> Self::Handle<T> {
        DebugHandle::from_raw_parts(handle, meta)
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
    ) -> crate::error::Result<Self::Handle<T>> {
        let handle = self.1.allocate_single::<T>(meta)?;
        let id = self.validate_alloc(true, S::cast(handle));
        Ok(DebugHandle { id, handle })
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.validate_dealloc(true, Self::cast(handle));
        // SAFETY: Shares our safety requirements
        unsafe { self.1.deallocate_single::<T>(handle.handle) }
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> crate::error::Result<Self::Handle<[T]>> {
        handle.try_map(|h| {
            // SAFETY: Shares our safety requirements
            unsafe { self.1.try_grow::<T>(h, capacity) }
        })
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> crate::error::Result<Self::Handle<[T]>> {
        handle.try_map(|h| {
            // SAFETY: Shares our safety requirements
            unsafe { self.1.try_shrink::<T>(h, capacity) }
        })
    }
}

// SAFETY: Debug delegates to another implementor of `Storage` which must uphold the guarantees
unsafe impl<S> MultiItemStorage for Debug<S>
where
    S: MultiItemStorage,
{
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> crate::error::Result<Self::Handle<T>> {
        let handle = self.1.allocate_single::<T>(meta)?;
        let id = self.validate_alloc(false, S::cast(handle));
        Ok(DebugHandle { id, handle })
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        self.validate_dealloc(false, Self::cast(handle));
        // SAFETY: Shares our safety requirements
        unsafe { self.1.deallocate(handle.handle) }
    }
}

impl<S> ExactSizeStorage for Debug<S>
where
    S: ExactSizeStorage,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        self.1.will_fit::<T>(meta)
    }

    fn max_range<T>(&self) -> usize {
        self.1.max_range::<T>()
    }
}

// unsafe impl<S> ClonesafeStorage for Debug<S> where S: ClonesafeStorage {}

// SAFETY: Debug delegates to another implementor of `Storage` which must uphold the guarantees
unsafe impl<S> LeaksafeStorage for Debug<S> where S: LeaksafeStorage {}

/*unsafe impl<S> FromLeakedStorage for Debug<S>
where
    S: FromLeakedStorage + ClonesafeStorage,
{
    unsafe fn unleak_ptr<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T> {
        let mut lock = self.0.lock();
        let id = lock.id;
        lock.id += 1;
        let handle = self.1.unleak_ptr(leaked);
        let out = DebugHandle { id, handle };
        self.0.lock().allocated_handles.push(self.cast(out));
        out
    }
}*/

mod private {
    use core::fmt;
    use super::*;

    /// Handle for a debug storage
    pub struct DebugHandle<S: Storage, T: ?Sized> {
        pub(super) id: usize,
        pub(super) handle: S::Handle<T>,
    }

    impl<S, T> DebugHandle<S, T>
    where
        S: Storage,
        T: ?Sized,
    {
        pub(super) fn map<U: ?Sized, F: FnOnce(S::Handle<T>) -> S::Handle<U>>(
            self,
            f: F,
        ) -> DebugHandle<S, U> {
            DebugHandle {
                id: self.id,
                handle: f(self.handle),
            }
        }

        pub(super) fn try_map<U: ?Sized, E, F: FnOnce(S::Handle<T>) -> Result<S::Handle<U>, E>>(
            self,
            f: F,
        ) -> Result<DebugHandle<S, U>, E> {
            Ok(DebugHandle {
                id: self.id,
                handle: f(self.handle)?,
            })
        }
    }

    impl<S, T> fmt::Debug for DebugHandle<S, T>
    where
        S: Storage,
        T: ?Sized,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("DebugHandle")
                .field("id", &self.id)
                .finish_non_exhaustive()
        }
    }

    impl<S, T> PartialEq for DebugHandle<S, T>
    where
        S: Storage,
        T: ?Sized,
    {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl<S: Storage, T: ?Sized> Clone for DebugHandle<S, T> {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl<S: Storage, T: ?Sized> Copy for DebugHandle<S, T> {}

    impl<S: Storage, T: ?Sized> Handle for DebugHandle<S, T> {
        type Addr = <S::Handle<T> as Handle>::Addr;
        type Target = T;
        type This<U: ?Sized> = DebugHandle<S, U>;

        fn from_raw_parts(
            handle: Self::This<()>,
            meta: <Self::Target as Pointee>::Metadata,
        ) -> Self {
            handle.map(|h| S::from_raw_parts(h, meta))
        }

        fn addr(self) -> Self::Addr {
            self.handle.addr()
        }

        fn metadata(self) -> <Self::Target as Pointee>::Metadata {
            self.handle.metadata()
        }

        fn with_addr(self, addr: Self::Addr) -> Self {
            self.map(|h| h.with_addr(addr))
        }

        fn map_addr(self, f: impl FnOnce(Self::Addr) -> Self::Addr) -> Self {
            self.map(|h| h.map_addr(f))
        }

        fn cast<U>(self) -> Self::This<U> {
            self.map(|h| S::cast::<T, U>(h))
        }

        fn cast_unsized<U>(self) -> Self::This<U>
        where
            U: ?Sized + Pointee<Metadata = <Self::Target as Pointee>::Metadata>,
        {
            self.map(|h| S::cast_unsized::<T, U>(h))
        }

        #[cfg(feature = "unsize")]
        fn coerce<U: ?Sized>(self) -> Self::This<U>
        where
            Self::Target: Unsize<U>,
        {
            self.map(|h| S::coerce::<T, U>(h))
        }
    }
}

use private::DebugHandle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline::SingleInline;

    fn storage() -> Debug<SingleInline<[usize; 16]>> {
        Debug::new(SingleInline::<[usize; 16]>::default())
    }

    #[test]
    fn test_correct() {
        let mut s = storage();

        let h1 = s.allocate_single::<()>(()).unwrap();
        unsafe { s.get(h1) };
        unsafe { s.deallocate_single(h1) };

        let h2 = s.allocate_single::<()>(()).unwrap();

        unsafe { s.get(h2) };
        unsafe { s.deallocate_single(h2) };
    }

    #[test]
    #[should_panic = "Called allocate_single without calling deallocate_single"]
    fn test_double_alloc() {
        let mut s = storage();

        s.allocate_single::<()>(()).unwrap();
        s.allocate_single::<()>(()).unwrap();
    }

    #[test]
    #[should_panic = "Called deallocate_single without first allocating"]
    fn test_dealloc_never_allocated() {
        let mut s1 = storage();
        let mut s2 = storage();

        let h1 = s1.allocate_single::<()>(()).unwrap();

        unsafe { s2.deallocate_single(h1) };
    }

    #[test]
    #[should_panic = "Called deallocate_single on the same handle twice"]
    fn test_double_free() {
        let mut s = storage();

        let h1 = s.allocate_single::<()>(()).unwrap();

        unsafe { s.deallocate_single(h1) };
        unsafe { s.deallocate_single(h1) };
    }

    #[test]
    #[should_panic = "Attempting to access allocation with deallocated handle"]
    fn test_get_deallocated() {
        let mut s = storage();

        let h1 = s.allocate_single::<()>(()).unwrap();

        unsafe { s.deallocate_single(h1) };

        unsafe { s.get(h1) };
    }

    #[test]
    #[should_panic = "Attempting to access allocation with never-allocated handle"]
    fn test_get_never_allocated() {
        let mut s1 = storage();
        let s2 = storage();

        let h1 = s1.allocate_single::<()>(()).unwrap();

        unsafe { s2.get(h1) };
    }

    #[test]
    #[should_panic = "Attempted to access single allocation with incorrect handle"]
    fn test_get_invalid() {
        let mut s = storage();

        let h1 = s.allocate_single::<()>(()).unwrap();

        unsafe { s.deallocate_single(h1) };

        s.allocate_single::<()>(()).unwrap();

        unsafe { s.get(h1) };
    }
}
