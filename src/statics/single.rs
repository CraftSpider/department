use core::{fmt, mem};
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};
use core::alloc::Layout;

use super::traits::StaticStorage;
use super::StorageCell;
use crate::base::{ExactSizeStorage, Storage, StorageSafe};
use crate::utils;
use crate::error::{Result, StorageError};

/// Static single-element storage implementation
pub struct SingleItem<S: 'static>(&'static StorageCell<S>);

impl<S: 'static> StaticStorage<S> for SingleItem<S> {
    fn take_cell(storage: &'static StorageCell<S>) -> SingleItem<S> {
        SingleItem(storage)
    }
}

unsafe impl<S> Storage for SingleItem<S>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = SingleStaticHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = self.0.as_ptr().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata=T::Metadata>>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        SingleStaticHandle(handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleStaticHandle(meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;
        Ok(SingleStaticHandle(meta))
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, _handle: Self::Handle<T>) {}

    unsafe fn try_grow<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.0);
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(SingleStaticHandle(capacity))
        } else {
            Err(StorageError::InsufficientSpace(new_layout.size(), Some(self.max_range::<T>())))
        }
    }

    unsafe fn try_shrink<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.0);
        Ok(SingleStaticHandle(capacity))
    }
}

unsafe impl<S> ExactSizeStorage for SingleItem<S>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = utils::layout_of::<T>(());
        mem::size_of::<S>() / layout.size()
    }
}

impl<S> fmt::Debug for SingleItem<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleElement").finish_non_exhaustive()
    }
}

impl<S> Drop for SingleItem<S> {
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

        let b = Box::<_, SingleItem<[usize; 4]>>::new_in([1, 2], FOO.claim());
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2]);
    }

    #[test]
    fn test_zst() {
        static FOO: StorageCell<[usize; 0]> = StorageCell::new([]);

        let b = Box::<(), SingleItem<[usize; 0]>>::new_in((), FOO.claim());

        assert_eq!(*b, ());
    }

    #[test]
    #[ignore = "This test is for human-readable output, and does not actually panic"]
    fn test_atomic() {
        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let mut handles = Vec::new();
        for i in 0..100 {
            handles.push(std::thread::spawn(move || {
                let storage = FOO.try_claim::<SingleItem<_>>();
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
