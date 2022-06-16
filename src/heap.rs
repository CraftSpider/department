//! Storage implementation which stores items in a virtual heap, either on the stack or in a static
//!
//! # Advantages
//! - No need for allocation
//! - Can provide 'heaps' which support any type of storage item (elements, ranges, etc)
//! - Implements many more extensions than inline or static storages
//!
//! # Disadvantages
//! - Increase binary or stack size
//!
//! # Examples
//!
//! ```
//! # use department::base::{ClonesafeStorage, Storage};
//! # use department::heap::VirtHeap;
//! # use department::backing::Backing;
//! # use department::rc::Rc;
//!
//! struct Node<'a, S: 'a + Storage + ClonesafeStorage> {
//!     inner: Rc<&'a str, S>,
//! }
//!
//! fn shared_item() {
//!     let rc_heap = VirtHeap::<Backing<16>, 1>::new();
//!     let rc1 = Rc::new_in("Hello World!", &rc_heap);
//!     let rc2 = Rc::clone(&rc1);
//!
//!     let node1 = Node { inner: rc1 };
//!     let node2 = Node { inner: rc2 };
//!
//!     // Use node as you see fit - any `Rc`s will live till the end of scope
//! }
//! ```

use core::alloc::Layout;
use core::cell::UnsafeCell;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ops::Range;
use core::ptr::{NonNull, Pointee};
use core::{mem, ptr};

use crate::base::{
    ClonesafeStorage, ExactSizeStorage, FromLeakedStorage, LeaksafeStorage, MultiItemStorage,
    Storage, StorageSafe,
};
use crate::error::{Result, StorageError};
use crate::utils;

/// Given a size, determine how many blocks are required to fit it
fn blocks<S>(size: usize) -> usize {
    size / mem::size_of::<S>()
}

/// Given a type and a length, determine how many blocks are needed to fit length instances
fn blocks_for<S, T>(capacity: usize) -> usize {
    (mem::size_of::<T>() * capacity) / mem::size_of::<S>()
}

/// A storage based on a variable (static or on the stack), supporting heap-like behavior but
/// compiled into the binary. Useful for environments with no allocator support but sufficient space
/// for either a larger binary or more stack usage.
///
/// Note that any items stored take at minimum one instance of `S` due to current limitations on
/// implementation.
#[derive(Debug)]
pub struct VirtHeap<S, const N: usize> {
    used: spin::Mutex<[bool; N]>,
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> VirtHeap<S, N>
where
    S: StorageSafe,
{
    /// Create a new heap
    pub const fn new() -> VirtHeap<S, N> {
        VirtHeap {
            used: spin::Mutex::new([false; N]),
            // SAFETY: The array contains only `MaybeUninit` values, so this is okay
            storage: UnsafeCell::new(unsafe {
                MaybeUninit::<[MaybeUninit<S>; N]>::uninit().assume_init()
            }),
        }
    }
}

impl<S, const N: usize> VirtHeap<S, N>
where
    S: StorageSafe,
{
    fn find_lock(&self, size: usize) -> Result<usize> {
        let mut used = self.used.lock();
        let open = self.find_open(&used, size)?;
        let start = open.start;
        self.lock_range(&mut used, open);
        Ok(start)
    }

    fn lock_range(&self, lock: &mut spin::MutexGuard<'_, [bool; N]>, range: Range<usize>) {
        lock[range].iter_mut().for_each(|i| {
            debug_assert!(!*i);
            *i = true
        });
    }

    fn unlock_range(&self, lock: &mut spin::MutexGuard<'_, [bool; N]>, range: Range<usize>) {
        lock[range].iter_mut().for_each(|i| {
            debug_assert!(*i);
            *i = false
        });
    }

    /// Attempt to find open space for an allocation of a given size.
    /// If size is zero, this returns a zero-sized range
    fn find_open(
        &self,
        lock: &spin::MutexGuard<'_, [bool; N]>,
        size: usize,
    ) -> Result<Range<usize>> {
        let blocks = blocks::<S>(size);

        if blocks == 0 {
            return Ok(0..0);
        }
        if blocks > N {
            return Err(StorageError::InsufficientSpace {
                expected: size,
                available: Some(mem::size_of::<S>() * N),
            });
        }

        lock.iter()
            // Count chains of `false` items
            .scan(0, |n, &v| {
                if v {
                    *n = 0
                } else {
                    *n += 1
                }
                Some(*n)
            })
            // Find the end point of a chain with the right size, if one exist
            .position(|count| count >= blocks)
            // Find the range of the desired chain
            .map(|end| {
                let start = end - (blocks - 1);
                start..(end + 1)
            })
            .ok_or(StorageError::NoSlots)
    }

    fn grow_in_place<T>(
        &self,
        handle: OffsetMetaHandle<[T]>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> bool {
        let mut used = self.used.lock();

        let old_blocks = blocks::<S>(old_layout.size());
        let new_blocks = blocks::<S>(new_layout.size());

        let after_old = (handle.offset() + old_blocks)..(handle.offset() + new_blocks);

        let has_space = used[after_old.clone()].iter().all(|&i| !i);

        if has_space {
            self.lock_range(&mut used, after_old);
        }

        has_space
    }

    fn grow_move<T>(
        &self,
        handle: <&Self as Storage>::Handle<[T]>,
        new_layout: Layout,
    ) -> Option<usize> {
        let mut used = self.used.lock();
        let old_range = handle.offset()..(handle.offset() + blocks_for::<S, T>(handle.metadata()));

        if handle.metadata() != 0 {
            self.unlock_range(&mut used, old_range.clone());
        }

        let new_range = match self.find_open(&used, new_layout.size()) {
            Ok(open) => open,
            Err(_) => {
                if handle.metadata() != 0 {
                    self.lock_range(&mut used, old_range);
                }
                return None;
            }
        };

        let new_start = new_range.start;
        self.lock_range(&mut used, new_range);

        // SAFETY: We only access slices of the mutex we have a lock on
        unsafe { &mut *self.storage.get() }.copy_within(old_range, new_start);

        Some(new_start)
    }
}

// SAFETY: Memory safety is uphold by the internal locks and check
unsafe impl<S, const N: usize> Storage for &VirtHeap<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = OffsetMetaHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        // SAFETY: We only access slices of the mutex this handle has a lock on
        let ptr = NonNull::new(ptr::addr_of_mut!((*self.storage.get())[handle.offset()]))
            .expect("Valid handle")
            .cast();
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
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.deallocate(handle)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.metadata());
        // We need to check if we can grow in-place. If not, then we need to see if we have any
        // open space for the new range, ignoring ourselves as we're allowed to overwrite that.
        let old_layout = Layout::array::<T>(handle.metadata()).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.grow_in_place(handle, old_layout, new_layout) {
            Ok(OffsetMetaHandle::from_offset_meta(
                handle.offset(),
                capacity,
            ))
        } else if let Some(new_start) = self.grow_move(handle, new_layout) {
            Ok(OffsetMetaHandle::from_offset_meta(new_start, capacity))
        } else {
            Err(StorageError::InsufficientSpace {
                expected: new_layout.size(),
                available: None,
            })
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.metadata());
        self.unlock_range(
            &mut self.used.lock(),
            (handle.offset() + capacity)..(handle.offset() + handle.metadata()),
        );
        Ok(OffsetMetaHandle::from_offset_meta(
            handle.offset(),
            capacity,
        ))
    }
}

// SAFETY: We can hold up to `N` items, internal locks and checks ensure memory safety
unsafe impl<S, const N: usize> MultiItemStorage for &VirtHeap<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);
        utils::validate_layout_for::<[S; N]>(layout)?;
        let start = self.find_lock(layout.size())?;
        Ok(OffsetMetaHandle::from_offset_meta(start, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(<Self as Storage>::get(self, handle).as_ref());
        let mut used = self.used.lock();
        self.unlock_range(
            &mut used,
            handle.offset()..(handle.offset() + blocks::<S>(layout.size())),
        );
    }
}

impl<S, const N: usize> ExactSizeStorage for &VirtHeap<S, N>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = Layout::new::<T>();
        (mem::size_of::<S>() * N) / layout.size()
    }
}

// SAFETY: All storages with the same heap backing can correctly handle each-other's allocations
unsafe impl<S, const N: usize> ClonesafeStorage for &VirtHeap<S, N> where S: StorageSafe {}

// SAFETY: Handles returned from a VirtHeap don't move and are valid until deallocated
unsafe impl<S, const N: usize> LeaksafeStorage for &VirtHeap<S, N> where S: StorageSafe {}

// SAFETY: A pointer leaked from a VirtHeap never got deallocated, so can be turned back into a
//         handle without issue
unsafe impl<S, const N: usize> FromLeakedStorage for &VirtHeap<S, N>
where
    S: StorageSafe,
{
    unsafe fn unleak_ptr<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T> {
        let meta = ptr::metadata(leaked);

        let offset: usize = leaked
            .cast::<S>()
            // We don't need a lock here because we never dereference the pointer
            .offset_from(self.storage.get() as *const S)
            .try_into()
            .unwrap();

        OffsetMetaHandle::from_offset_meta(offset, meta)
    }
}

// SAFETY: This type only accesses the inner cell when atomically claimed
unsafe impl<S: Send + StorageSafe, const N: usize> Send for VirtHeap<S, N> {}
// SAFETY: This type only accesses the inner cell when atomically claimed
unsafe impl<S: Sync + StorageSafe, const N: usize> Sync for VirtHeap<S, N> {}

use crate::handles::OffsetMetaHandle;

#[cfg(test)]
mod tests {
    use crate::boxed::Box;
    use crate::collections::Vec;

    use super::*;

    #[test]
    fn test_box() {
        static HEAP: VirtHeap<usize, 4> = VirtHeap::new();
        let b = Box::new_in([1, 2], &HEAP);
        let b2 = b.coerce::<[i32]>();

        assert_eq!(&*b2, &[1, 2]);
    }

    #[test]
    fn test_multi_box() {
        static HEAP: VirtHeap<usize, 16> = VirtHeap::new();
        let b1 = Box::new_in([1, 2], &HEAP);
        let b2 = Box::new_in([3, 4], &HEAP);
        let b3 = Box::new_in([5, 6], &HEAP);
        let b4 = Box::new_in([7, 8], &HEAP);

        assert_eq!(*b1, [1, 2]);
        assert_eq!(*b2, [3, 4]);
        assert_eq!(*b3, [5, 6]);
        assert_eq!(*b4, [7, 8]);
    }

    #[test]
    fn test_vec() {
        static HEAP: VirtHeap<usize, 16> = VirtHeap::new();

        let mut v = Vec::new_in(&HEAP);
        v.push(1);
        v.push(2);

        assert_eq!(&*v, &[1, 2]);
    }

    #[test]
    fn test_multi_vec() {
        static HEAP: VirtHeap<usize, 16> = VirtHeap::new();

        let mut v1 = Vec::new_in(&HEAP);
        let mut v2 = Vec::new_in(&HEAP);
        let mut v3 = Vec::new_in(&HEAP);
        let mut v4 = Vec::new_in(&HEAP);

        v1.extend([1, 2]);
        v2.extend([3, 4]);
        v3.extend([5, 6]);
        v4.extend([7, 8]);

        v1.extend([9, 10, 11, 12, 13, 14, 15, 16]);

        assert_eq!(&*v1, &[1, 2, 9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(&*v2, &[3, 4]);
        assert_eq!(&*v3, &[5, 6]);
        assert_eq!(&*v4, &[7, 8]);
    }

    #[test]
    fn test_size() {
        static HEAP: VirtHeap<u8, 4> = VirtHeap::new();

        type Box<T> = crate::boxed::Box<T, &'static VirtHeap<u8, 4>>;

        Box::<[u8; 4]>::try_new_in([1, 2, 3, 4], &HEAP).unwrap();
        Box::<[u8; 8]>::try_new_in([1, 2, 3, 4, 5, 6, 7, 8], &HEAP).unwrap_err();
    }

    #[test]
    fn test_align() {
        static FOO1: VirtHeap<u8, 4> = VirtHeap::new();
        static FOO2: VirtHeap<u16, 4> = VirtHeap::new();
        static FOO4: VirtHeap<u32, 4> = VirtHeap::new();
        static FOO8: VirtHeap<u64, 4> = VirtHeap::new();

        type Box<T, S> = crate::boxed::Box<T, &'static VirtHeap<S, 4>>;

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

        Box::<_, u8>::try_new_in(Align1, &FOO1).unwrap();
        Box::<_, u8>::try_new_in(Align2, &FOO1).unwrap_err();
        Box::<_, u8>::try_new_in(Align4, &FOO1).unwrap_err();
        Box::<_, u8>::try_new_in(Align8, &FOO1).unwrap_err();

        Box::<_, u16>::try_new_in(Align1, &FOO2).unwrap();
        Box::<_, u16>::try_new_in(Align2, &FOO2).unwrap();
        Box::<_, u16>::try_new_in(Align4, &FOO2).unwrap_err();
        Box::<_, u16>::try_new_in(Align8, &FOO2).unwrap_err();

        Box::<_, u32>::try_new_in(Align1, &FOO4).unwrap();
        Box::<_, u32>::try_new_in(Align2, &FOO4).unwrap();
        Box::<_, u32>::try_new_in(Align4, &FOO4).unwrap();
        Box::<_, u32>::try_new_in(Align8, &FOO4).unwrap_err();

        Box::<_, u64>::try_new_in(Align1, &FOO8).unwrap();
        Box::<_, u64>::try_new_in(Align2, &FOO8).unwrap();
        Box::<_, u64>::try_new_in(Align4, &FOO8).unwrap();
        Box::<_, u64>::try_new_in(Align8, &FOO8).unwrap();
    }

    #[test]
    fn test_leak() {
        static HEAP: VirtHeap<usize, 16> = VirtHeap::new();

        let v1 = Box::new_in(1, &HEAP);

        let i = Box::leak(v1);

        assert_eq!(*i, 1);
        *i = -1;
        assert_eq!(*i, -1);

        let v1 = unsafe { Box::from_raw_in(i, &HEAP) };

        assert_eq!(*v1, -1);
    }

    #[test]
    fn test_non_static() {
        let heap: VirtHeap<u32, 4> = VirtHeap::new();
        Box::new_in(1, &heap);
    }
}
