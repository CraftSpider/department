use core::alloc::Layout;
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem};

use super::traits::StaticStorage;
use super::StorageCell;
use crate::base::{ExactSizeStorage, Storage, StorageSafe};
use crate::error::{Result, StorageError};
use crate::utils;

/// Static single-element storage implementation
pub struct SingleStatic<S: 'static>(&'static StorageCell<S>);

impl<S: 'static> StaticStorage<S> for SingleStatic<S> {
    fn take_cell(storage: &'static StorageCell<S>) -> SingleStatic<S> {
        SingleStatic(storage)
    }
}

unsafe impl<S> Storage for SingleStatic<S>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = SingleStaticHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = self.0.as_ptr().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        SingleStaticHandle(handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleStaticHandle(meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;
        Ok(SingleStaticHandle(meta))
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, _handle: Self::Handle<T>) {}

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.0);
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(SingleStaticHandle(capacity))
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
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.0);
        Ok(SingleStaticHandle(capacity))
    }
}

impl<S> ExactSizeStorage for SingleStatic<S>
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

impl<S> fmt::Debug for SingleStatic<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleElement").finish_non_exhaustive()
    }
}

impl<S> Drop for SingleStatic<S> {
    fn drop(&mut self) {
        self.0.release()
    }
}

pub struct SingleStaticHandle<T: ?Sized + Pointee>(T::Metadata);

impl<T: ?Sized + Pointee> Clone for SingleStaticHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for SingleStaticHandle<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxed::Box;

    use core::time::Duration;

    #[test]
    fn test_box() {
        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let b = Box::<_, SingleStatic<[usize; 4]>>::new_in([1, 2], FOO.claim());
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2]);
    }

    #[test]
    fn test_size() {
        static FOO: StorageCell<[u8; 4]> = StorageCell::new([0; 4]);

        type Box<T> = crate::boxed::Box<T, SingleStatic<[u8; 4]>>;

        Box::<[u8; 4]>::try_new_in([1, 2, 3, 4], FOO.claim()).unwrap();
        Box::<[u8; 8]>::try_new_in([1, 2, 3, 4, 5, 6, 7, 8], FOO.claim()).unwrap_err();
    }

    #[test]
    fn test_align() {
        static FOO1: StorageCell<[u8; 4]> = StorageCell::new([0; 4]);
        static FOO2: StorageCell<[u16; 4]> = StorageCell::new([0; 4]);
        static FOO4: StorageCell<[u32; 4]> = StorageCell::new([0; 4]);
        static FOO8: StorageCell<[u64; 4]> = StorageCell::new([0; 4]);

        type Box<T, S> = crate::boxed::Box<T, SingleStatic<[S; 4]>>;

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

        Box::<_, u8>::try_new_in(Align1, FOO1.claim()).unwrap();
        Box::<_, u8>::try_new_in(Align2, FOO1.claim()).unwrap_err();
        Box::<_, u8>::try_new_in(Align4, FOO1.claim()).unwrap_err();
        Box::<_, u8>::try_new_in(Align8, FOO1.claim()).unwrap_err();

        Box::<_, u16>::try_new_in(Align1, FOO2.claim()).unwrap();
        Box::<_, u16>::try_new_in(Align2, FOO2.claim()).unwrap();
        Box::<_, u16>::try_new_in(Align4, FOO2.claim()).unwrap_err();
        Box::<_, u16>::try_new_in(Align8, FOO2.claim()).unwrap_err();

        Box::<_, u32>::try_new_in(Align1, FOO4.claim()).unwrap();
        Box::<_, u32>::try_new_in(Align2, FOO4.claim()).unwrap();
        Box::<_, u32>::try_new_in(Align4, FOO4.claim()).unwrap();
        Box::<_, u32>::try_new_in(Align8, FOO4.claim()).unwrap_err();

        Box::<_, u64>::try_new_in(Align1, FOO8.claim()).unwrap();
        Box::<_, u64>::try_new_in(Align2, FOO8.claim()).unwrap();
        Box::<_, u64>::try_new_in(Align4, FOO8.claim()).unwrap();
        Box::<_, u64>::try_new_in(Align8, FOO8.claim()).unwrap();
    }

    #[test]
    fn test_zst() {
        static FOO: StorageCell<[usize; 0]> = StorageCell::new([]);

        let b = Box::<(), SingleStatic<[usize; 0]>>::new_in((), FOO.claim());

        assert_eq!(*b, ());
    }

    #[test]
    #[ignore = "This test is for human-readable output, and does not actually panic"]
    fn test_atomic() {
        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let mut handles = Vec::new();
        for i in 0..100 {
            handles.push(std::thread::spawn(move || {
                let storage = FOO.try_claim::<SingleStatic<_>>();
                if storage.is_some() {
                    println!("Thread {} claimed storage", i);
                    std::thread::sleep(Duration::from_millis(1));
                } else {
                    println!("Thread {} couldn't claim storage", i);
                }
                core::mem::drop(storage);
                println!("Thread {} released storage", i);
            }));
        }

        handles
            .into_iter()
            .for_each(|handle| handle.join().unwrap())
    }
}
