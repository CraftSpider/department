
use core::ptr::{Pointee, NonNull};
use core::marker::Unsize;
use core::fmt;

use super::StorageCell;
use super::traits::StaticStorage;
use crate::utils;
use crate::traits::{SingleElementStorage, ElementStorage};

pub struct SingleElement<S: 'static>(&'static StorageCell<S>);

impl<S: 'static> StaticStorage<S> for SingleElement<S> {
    fn take_cell(storage: &'static StorageCell<S>) -> SingleElement<S> {
        SingleElement(storage)
    }
}

impl<S> ElementStorage for SingleElement<S> {
    type Handle<T: ?Sized + Pointee> = SingleElementHandle<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let ptr: NonNull<()> = NonNull::new(self.0.as_ptr()).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.0)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        SingleElementHandle(meta)
    }
}

impl<S> SingleElementStorage for SingleElement<S> {
    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> crate::traits::Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;
        Ok(SingleElementHandle(meta))
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, _handle: Self::Handle<T>) {}
}

impl<S> fmt::Debug for SingleElement<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleElement").finish_non_exhaustive()
    }
}

impl<S> Drop for SingleElement<S> {
    fn drop(&mut self) {
        self.0.release()
    }
}

pub struct SingleElementHandle<T: ?Sized + Pointee>(T::Metadata);

impl<T: ?Sized + Pointee> Clone for SingleElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for SingleElementHandle<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxed::Box;

    use core::time::Duration;

    #[test]
    fn test_box() {
        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let b = Box::<_, SingleElement<[usize; 4]>>::new_in([1, 2], FOO.claim());
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2]);
    }

    #[test]
    #[ignore = "This test is for human-readable output, and does not actually panic"]
    fn test_atomic() {

        static FOO: StorageCell<[usize; 4]> = StorageCell::new([0; 4]);

        let mut handles = Vec::new();
        for i in 0..100 {
            handles.push(std::thread::spawn(move || {
                let storage = FOO.try_claim::<SingleElement<_>>();
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
