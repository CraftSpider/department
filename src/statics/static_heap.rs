use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ops::Range;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem, ptr};

use crate::base::{
    Storage, MultiItemStorage, ExactSizeStorage, LeaksafeStorage, FromLeakedPtrStorage, StorageSafe
};
use crate::error::{Result, StorageError};
use crate::utils;

fn blocks<S>(size: usize) -> usize {
    (size + 3) / mem::size_of::<S>()
}

fn blocks_for<S, T>(capacity: usize) -> usize {
    (mem::size_of::<T>() * capacity + 3) / mem::size_of::<S>()
}

// TODO: Safety comments for this. There may be race conditions with the UnsafeCell, should
//       find a way to better architect for easier safety reasoning

/// A storage based on a static variable, supporting heap-like behavior but compiled into
/// the binary. Useful for environments with no allocator support but sufficient space for a
/// slightly larger program.
pub struct StaticHeap<S, const N: usize> {
    used: spin::Mutex<[bool; N]>,
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> StaticHeap<S, N> {
    /// Create a new static heap
    pub const fn new() -> StaticHeap<S, N> {
        StaticHeap {
            used: spin::Mutex::new([false; N]),
            storage: UnsafeCell::new(MaybeUninit::uninit_array()),
        }
    }
}

impl<S, const N: usize> StaticHeap<S, N>
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
        if blocks >= N {
            return Err(StorageError::InsufficientSpace(
                size,
                Some(mem::size_of::<S>() * N),
            ));
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
        handle: HeapHandle<[T]>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> bool {
        let mut used = self.used.lock();

        let old_blocks = blocks::<S>(old_layout.size());
        let new_blocks = blocks::<S>(new_layout.size());

        let after_old = (handle.0 + old_blocks)..(handle.0 + new_blocks);

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
        let old_range = handle.0..(handle.0 + blocks_for::<S, T>(handle.1));

        if handle.1 != 0 {
            self.unlock_range(&mut used, old_range.clone());
        }

        let new_range = match self.find_open(&used, new_layout.size()) {
            Ok(open) => open,
            Err(_) => {
                if handle.1 != 0 {
                    self.lock_range(&mut used, old_range);
                }
                return None;
            }
        };

        let new_start = new_range.start;
        self.lock_range(&mut used, new_range);

        // SAFETY: Satisfies internal requirement that the cell is only accessed while the mutex
        //         is locked
        unsafe { &mut *self.storage.get() }.copy_within(old_range, new_start);

        Some(new_start)
    }
}

unsafe impl<S, const N: usize> Storage for &StaticHeap<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = HeapHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr = NonNull::new(ptr::addr_of_mut!((*self.storage.get())[handle.0]))
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata=T::Metadata>>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        HeapHandle(handle.0, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        let element = <Self as Storage>::get(self, handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        HeapHandle(handle.0, meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.deallocate(handle)
    }

    unsafe fn try_grow<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.1);
        // We need to check if we can grow in-place. If not, then we need to see if we have any
        // open space for the new range, ignoring ourselves as we're allowed to overwrite that.
        let old_layout = Layout::array::<T>(handle.1).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.grow_in_place(handle, old_layout, new_layout) {
            Ok(HeapHandle(handle.0, capacity))
        } else if let Some(new_start) = self.grow_move(handle, new_layout) {
            Ok(HeapHandle(new_start, capacity))
        } else {
            Err(StorageError::InsufficientSpace(new_layout.size(), None))
        }
    }

    unsafe fn try_shrink<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.1);
        self.unlock_range(
            &mut self.used.lock(),
            (handle.0 + capacity)..(handle.0 + handle.1),
        );
        Ok(HeapHandle(handle.0, capacity))
    }
}

unsafe impl<S, const N: usize> MultiItemStorage for &StaticHeap<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);
        utils::validate_layout_for::<[S; N]>(layout)?;
        let start = self.find_lock(layout.size())?;
        Ok(HeapHandle(start, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(<Self as Storage>::get(self, handle).as_ref());
        let mut used = self.used.lock();
        self.unlock_range(&mut used, handle.0..(handle.0 + blocks::<S>(layout.size())));
    }
}

unsafe impl<S, const N: usize> ExactSizeStorage for &StaticHeap<S, N>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = utils::layout_of::<T>(());
        (mem::size_of::<S>() * N) / layout.size()
    }
}

// SAFETY: Handles returned from a StaticHeap don't move and are valid until deallocated
unsafe impl<S, const N: usize> LeaksafeStorage for &'static StaticHeap<S, N> where S: StorageSafe {}

unsafe impl<S, const N: usize> FromLeakedPtrStorage for &'static StaticHeap<S, N>
where
    S: StorageSafe,
{
    unsafe fn unleak<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T> {
        let meta = ptr::metadata(leaked);

        let offset: usize = leaked
            .cast::<S>()
            .offset_from(self.storage.get() as *const S)
            .try_into()
            .unwrap();

        HeapHandle(offset, meta)
    }
}

// SAFETY: This type only accesses the inner cell when atomically claimed
unsafe impl<S: Send + StorageSafe, const N: usize> Send for StaticHeap<S, N> {}
// SAFETY: This type only accesses the inner cell when atomically claimed
unsafe impl<S: Sync + StorageSafe, const N: usize> Sync for StaticHeap<S, N> {}

pub struct HeapHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized> Clone for HeapHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for HeapHandle<T> {}

impl<T: ?Sized> fmt::Debug for HeapHandle<T>
where
    <T as Pointee>::Metadata: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HeapHandle")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::boxed::Box;
    use crate::collections::Vec;

    use super::*;

    #[test]
    fn test_box() {
        static HEAP: StaticHeap<usize, 4> = StaticHeap::new();
        let b = Box::new_in([1, 2], &HEAP);
        let b2 = b.coerce::<[i32]>();

        assert_eq!(&*b2, &[1, 2]);
    }

    #[test]
    fn test_multi_box() {
        static HEAP: StaticHeap<usize, 16> = StaticHeap::new();
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
        static HEAP: StaticHeap<usize, 16> = StaticHeap::new();

        let mut v = Vec::new_in(&HEAP);
        v.push(1);
        v.push(2);

        assert_eq!(&*v, &[1, 2]);
    }

    #[test]
    fn test_multi_vec() {
        static HEAP: StaticHeap<usize, 16> = StaticHeap::new();

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
    fn test_leak() {
        static HEAP: StaticHeap<usize, 16> = StaticHeap::new();

        let v1 = Box::new_in(1, &HEAP);

        let i = v1.leak();

        assert_eq!(*i, 1);
        *i = -1;
        assert_eq!(*i, -1);

        let v1 = unsafe { Box::unleak_in(i, &HEAP) };

        assert_eq!(*v1, -1);
    }
}
