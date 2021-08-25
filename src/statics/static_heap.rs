use core::{mem, ptr};
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ops::Range;
use core::ptr::{NonNull, Pointee};

use crate::error::{Result, StorageError};
use crate::traits::{
    ElementStorage, MultiElementStorage, MultiRangeStorage, RangeStorage,
    SingleElementStorage, SingleRangeStorage,
};
use crate::utils;

fn blocks<S>(size: usize) -> usize {
    (size + 3) / mem::size_of::<S>()
}

pub struct StaticHeap<S, const N: usize> {
    used: spin::Mutex<[bool; N]>,
    storage: UnsafeCell<[MaybeUninit<S>; N]>,
}

impl<S, const N: usize> StaticHeap<S, N> {
    pub const fn new() -> StaticHeap<S, N> {
        StaticHeap {
            used: spin::Mutex::new([false; N]),
            storage: UnsafeCell::new(MaybeUninit::uninit_array::<N>()),
        }
    }

    fn try_lock(&self, size: usize) -> Result<usize> {
        let mut used = self.used.lock();
        let open = self.find_open(&used, size)?;
        let start = open.start;

        used[open].iter_mut().for_each(|i| *i = true);

        Ok(start)
    }

    fn unlock(&self, start: usize, size: usize) {
        for i in &mut self.used.lock()[start..(start + blocks::<S>(size))] {
            *i = false
        }
    }

    fn find_open(
        &self,
        lock: &spin::MutexGuard<'_, [bool; N]>,
        size: usize,
    ) -> Result<Range<usize>> {
        if blocks::<S>(size) > N {
            return Err(StorageError::InsufficientSpace(
                size,
                Some(mem::size_of::<S>() * N),
            ));
        }

        lock.iter()
            .scan(0, |n, &v| {
                if !v {
                    *n += 1
                } else {
                    *n = 0
                }
                Some(*n)
            })
            .position(|n| n >= blocks::<S>(size))
            .map(|n| (n - (usize::max(blocks::<S>(size), 1) - 1)))
            .map(|start| start..(start + blocks::<S>(size)))
            .ok_or(StorageError::NoSlots)
    }

    fn grow_in_place<T>(
        &self,
        handle: <&Self as RangeStorage>::Handle<T>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> bool {
        let mut used = self.used.lock();

        let old_blocks = blocks::<S>(old_layout.size());
        let new_blocks = blocks::<S>(new_layout.size());

        let has_space = used[(handle.0 + old_blocks)..(handle.0 + new_blocks)]
            .iter()
            .all(|&i| i == false);

        if has_space {
            used[(handle.0 + old_blocks)..(handle.0 + new_blocks)]
                .iter_mut()
                .for_each(|i| *i = true);
        }

        has_space
    }

    fn grow_move<T>(
        &self,
        handle: <&Self as RangeStorage>::Handle<T>,
        new_layout: Layout,
    ) -> Option<usize> {
        let mut lock = self.used.lock();

        lock[handle.0..(handle.0 + handle.1)]
            .iter_mut()
            .for_each(|i| *i = false);

        let new_range = match self.find_open(&lock, new_layout.size()) {
            Ok(open) => open,
            Err(_) => {
                lock[handle.0..(handle.0 + handle.1)]
                    .iter_mut()
                    .for_each(|i| *i = true);
                return None;
            }
        };

        Some(new_range.start)
    }
}

impl<S, const N: usize> ElementStorage for &StaticHeap<S, N> {
    type Handle<T: ?Sized + Pointee> = HeapHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr = NonNull::new(ptr::addr_of_mut!((*self.storage.get())[handle.0]))
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = <Self as ElementStorage>::get(self, handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        HeapHandle(handle.0, meta)
    }
}

impl<S, const N: usize> SingleElementStorage for &StaticHeap<S, N> {
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        <Self as MultiElementStorage>::allocate(self, meta)
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        <Self as MultiElementStorage>::deallocate(self, handle)
    }
}

impl<S, const N: usize> MultiElementStorage for &StaticHeap<S, N> {
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);
        utils::validate_layout_for::<[S; N]>(layout)?;
        let start = self.try_lock(layout.size())?;
        Ok(HeapHandle(start, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(<Self as ElementStorage>::get(self, handle).as_ref());
        self.unlock(handle.0, layout.size());
    }
}

impl<S, const N: usize> RangeStorage for &StaticHeap<S, N> {
    type Handle<T> = HeapHandle<[T]>;

    fn maximum_capacity<T>(&self) -> usize {
        (mem::size_of::<S>() * N) / mem::size_of::<T>()
    }

    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        let ptr = NonNull::new(ptr::addr_of_mut!((*self.storage.get())[handle.0]))
            .expect("Valid handle")
            .cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> Result<Self::Handle<T>> {
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

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> Result<Self::Handle<T>> {
        debug_assert!(capacity <= handle.1);

        self.used
            .lock()[(handle.0 + capacity)..(handle.0 + handle.1)]
            .iter_mut()
            .for_each(|i| *i = false);

        Ok(HeapHandle(handle.0, capacity))
    }
}

impl<S, const N: usize> SingleRangeStorage for &StaticHeap<S, N> {
    fn allocate_single<T>(&mut self, capacity: usize) -> Result<Self::Handle<T>> {
        <Self as MultiRangeStorage>::allocate(self, capacity)
    }

    unsafe fn deallocate_single<T>(&mut self, handle: Self::Handle<T>) {
        <Self as MultiRangeStorage>::deallocate(self, handle)
    }
}

impl<S, const N: usize> MultiRangeStorage for &StaticHeap<S, N> {
    fn allocate<T>(&mut self, capacity: usize) -> Result<Self::Handle<T>> {
        let layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;
        utils::validate_layout_for::<[S; N]>(layout)?;
        let start = self.try_lock(layout.size())?;
        Ok(HeapHandle(start, capacity))
    }

    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(<Self as ElementStorage>::get(self, handle).as_ref());
        self.unlock(handle.0, layout.size());
    }
}

unsafe impl<S: Send, const N: usize> Send for StaticHeap<S, N> {}
unsafe impl<S: Sync, const N: usize> Sync for StaticHeap<S, N> {}

pub struct HeapHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized> Clone for HeapHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for HeapHandle<T> {}

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

        assert_eq!(&*v1, &[1, 2]);
        assert_eq!(&*v2, &[3, 4]);
        assert_eq!(&*v3, &[5, 6]);
        assert_eq!(&*v4, &[7, 8]);
    }
}
