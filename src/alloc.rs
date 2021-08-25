use core::alloc::{Allocator, Layout};
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use rs_alloc::alloc::Global;

use crate::error::StorageError;
use crate::traits::{
    ElementStorage, MultiElementStorage, MultiRangeStorage, RangeStorage, SingleElementStorage,
    SingleRangeStorage,
};
use crate::utils;

pub type GlobalAlloc = Alloc<Global>;

#[derive(Copy, Clone)]
pub struct Alloc<A: Allocator>(A);

impl<A: Allocator> Alloc<A> {
    pub fn new(alloc: A) -> Alloc<A> {
        Alloc(alloc)
    }
}

impl Alloc<Global> {
    pub fn global() -> Alloc<Global> {
        Alloc(Global)
    }
}

impl<A: Allocator + Default> Default for Alloc<A> {
    fn default() -> Self {
        Alloc(A::default())
    }
}

impl<A: Allocator> ElementStorage for Alloc<A> {
    type Handle<T: ?Sized + Pointee> = NonNull<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        handle
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        handle
    }
}

impl<A: Allocator> SingleElementStorage for Alloc<A> {
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> crate::error::Result<Self::Handle<T>> {
        <Self as MultiElementStorage>::allocate(self, meta)
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        <Self as MultiElementStorage>::deallocate(self, handle)
    }
}

impl<A: Allocator> MultiElementStorage for Alloc<A> {
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> crate::error::Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);

        let allocated: NonNull<()> = self
            .0
            .allocate(layout)
            .map_err(|_| StorageError::InsufficientSpace(layout.size(), None))?
            .cast();

        Ok(NonNull::from_raw_parts(allocated, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(handle.as_ref());
        self.0.deallocate(handle.cast(), layout);
    }
}

impl<A: Allocator> RangeStorage for Alloc<A> {
    type Handle<T> = NonNull<[MaybeUninit<T>]>;

    fn maximum_capacity<T>(&self) -> usize {
        usize::MAX
    }

    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]> {
        handle
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> crate::error::Result<Self::Handle<T>> {
        let old_len = handle.as_ref().len();

        let old_layout = Layout::array::<T>(old_len).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        let new_ptr = self
            .0
            .grow(handle.cast(), old_layout, new_layout)
            // This may actually be unimplemented or other, but we're making an educated guess
            .map_err(|_| StorageError::InsufficientSpace(new_layout.size(), None))?;

        Ok(NonNull::from_raw_parts(new_ptr.cast(), capacity))
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> crate::error::Result<Self::Handle<T>> {
        let old_len = handle.as_ref().len();

        let old_layout = Layout::array::<T>(old_len).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        let new_ptr = self
            .0
            .shrink(handle.cast(), old_layout, new_layout)
            // Shrinking should... probably only fail if it's not a thing?
            .map_err(|_| StorageError::Unimplemented)?;

        Ok(NonNull::from_raw_parts(new_ptr.cast(), capacity))
    }
}

impl<A: Allocator> SingleRangeStorage for Alloc<A> {
    fn allocate_single<T>(&mut self, capacity: usize) -> crate::error::Result<Self::Handle<T>> {
        <Self as MultiRangeStorage>::allocate(self, capacity)
    }

    unsafe fn deallocate_single<T>(&mut self, handle: Self::Handle<T>) {
        <Self as MultiRangeStorage>::deallocate(self, handle)
    }
}

impl<A: Allocator> MultiRangeStorage for Alloc<A> {
    fn allocate<T>(&mut self, capacity: usize) -> crate::error::Result<Self::Handle<T>> {
        let layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;
        let pointer = self
            .0
            .allocate(layout)
            .map_err(|_| StorageError::InsufficientSpace(layout.size(), None))?;
        Ok(NonNull::from_raw_parts(pointer.cast(), capacity))
    }

    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>) {
        let old_len = handle.as_ref().len();

        let layout = Layout::array::<T>(old_len).expect("Valid handle");
        let pointer = handle;

        self.0.deallocate(pointer.cast(), layout)
    }
}

#[cfg(test)]
mod tests {
    use crate::boxed::Box;
    use crate::collections::Vec;

    use super::*;

    #[test]
    fn test_box() {
        let b = Box::<_, Alloc<Global>>::new([1, 2, 3, 4]);
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_vec() {
        let mut v = Vec::<_, Alloc<Global>>::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v.push(4);

        assert_eq!(&*v, &[1, 2, 3, 4]);
    }
}
