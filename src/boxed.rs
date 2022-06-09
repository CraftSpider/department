//! A storage-based implementation of [`std::boxed`]

use core::alloc::Layout;
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::marker::Unsize;
use core::mem::ManuallyDrop;
use core::ops::{CoerceUnsized, Deref, DerefMut};
use core::ptr::Pointee;
use core::{fmt, mem, ptr};

use crate::base::{LeaksafeStorage, SingleElementStorage};

/// Storage-based implementation of [`Box`](std::boxed::Box).
///
/// Note that unsizing coercion currently isn't expressive enough to support all storage types,
/// so the implementation provides a `coerce` method which can be used to emulate the same
/// functionality.
pub struct Box<T: ?Sized + Pointee, S: SingleElementStorage> {
    handle: S::Handle<T>,
    storage: ManuallyDrop<S>,
}

impl<T, S> Box<T, S>
where
    T: Pointee,
    S: SingleElementStorage + Default,
{
    /// Create a new [`Box`] containing the provided value, creating a default instance of the
    /// desired storage.
    ///
    /// # Panics
    ///
    /// If the storage fails to allocate for any reason
    pub fn new(val: T) -> Box<T, S> {
        let mut storage = S::default();
        Box {
            handle: storage
                .create_single(val)
                .unwrap_or_else(|(e, _)| panic!("{}", e)),
            storage: ManuallyDrop::new(storage),
        }
    }

    /// Attempt to create a new [`Box`] containing the provided value, creating a default instance
    /// of the desired storage.
    pub fn try_new(val: T) -> Result<Box<T, S>, T> {
        let mut storage = S::default();
        Ok(Box {
            handle: storage.create_single(val).map_err(|(_, t)| t)?,
            storage: ManuallyDrop::new(storage),
        })
    }
}

impl<T, S> Box<T, S>
where
    T: Pointee,
    S: SingleElementStorage,
{
    /// Create a new [`Box`] containing the provided value, in the provided storage.
    ///
    /// # Panics
    ///
    /// If the storage fails to allocate for any reason
    pub fn new_in(val: T, mut storage: S) -> Box<T, S> {
        Box {
            handle: storage
                .create_single(val)
                .unwrap_or_else(|(e, _)| panic!("{}", e)),
            storage: ManuallyDrop::new(storage),
        }
    }

    /// Attempt to create a new [`Box`] containing the provided value, in the provided storage.
    pub fn try_new_in(val: T, mut storage: S) -> Result<Box<T, S>, (T, S)> {
        let handle = match storage.create_single(val) {
            Ok(handle) => handle,
            Err((_, val)) => return Err((val, storage)),
        };

        Ok(Box {
            handle,
            storage: ManuallyDrop::new(storage),
        })
    }
}

impl<T, S> Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    /// Attempt to move the value from this `Box` into another one using a different backing
    /// storage. In case of failure, the original `Box` is returned unchanged.
    pub fn try_in<Ns>(mut self, mut new_storage: Ns) -> Result<Box<T, Ns>, (Box<T, S>, Ns)>
    where
        Ns: SingleElementStorage,
    {
        let layout = Layout::for_value(&*self);

        // SAFETY: Our handle is guaranteed valid by internal invariant
        let (old_ptr, meta) = unsafe { self.storage.get(self.handle).to_raw_parts() };
        let new_handle = match new_storage.allocate_single::<T>(meta) {
            Ok(handle) => handle,
            Err(_) => return Err((self, new_storage)),
        };

        // SAFETY: New handle is valid because allocate just succeeded
        let new_ptr = unsafe { new_storage.get(new_handle).to_raw_parts().0 };

        // SAFETY: Handles are from different allocations
        unsafe {
            ptr::copy_nonoverlapping(
                old_ptr.as_ptr().cast::<u8>(),
                new_ptr.as_ptr().cast::<u8>(),
                layout.size(),
            )
        };

        // SAFETY: Our handle is guaranteed valid by internal invariant
        unsafe { self.storage.deallocate_single(self.handle) };
        // SAFETY: We consume self, so no one will touch us after this
        unsafe { ManuallyDrop::drop(&mut self.storage) };
        // Don't run drop as we manually deallocated
        mem::forget(self);

        Ok(Box {
            handle: new_handle,
            storage: ManuallyDrop::new(new_storage),
        })
    }

    /// 'Leak' this box, returning a reference to the inner data that will never be deallocated
    pub fn leak<'a>(self) -> &'a mut T
    where
        S: LeaksafeStorage,
    {
        // SAFETY: Our handle is guaranteed valid by internal invariant
        let mut out = unsafe { self.storage.get(self.handle) };
        mem::forget(self);
        // SAFETY: `LeaksafeStorage` bound allows pointers to allocator to live arbitrarily long
        unsafe { out.as_mut() }
    }

    /// Perform an unsizing operation on `self`. A temporary solution to limitations with
    /// manual unsizing.
    pub fn coerce<U: ?Sized>(mut self) -> Box<U, S>
    where
        T: Unsize<U>,
    {
        // SAFETY: Our handle is guaranteed valid by internal invariant
        let handle = unsafe { self.storage.coerce::<_, U>(self.handle) };
        // SAFETY: We consume self, so no one will touch us after this
        let storage = unsafe { ManuallyDrop::take(&mut self.storage) };
        // Don't run drop for the old handle
        mem::forget(self);
        Box {
            handle,
            storage: ManuallyDrop::new(storage),
        }
    }
}

impl<T, S> fmt::Debug for Box<T, S>
where
    T: ?Sized + fmt::Debug,
    S: SingleElementStorage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ref())
    }
}

impl<T, S> fmt::Display for Box<T, S>
where
    T: ?Sized + fmt::Display,
    S: SingleElementStorage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl<T, U, S> CoerceUnsized<Box<U, S>> for Box<T, S>
where
    T: ?Sized + Pointee,
    U: ?Sized + Pointee,
    S: SingleElementStorage,
    S::Handle<T>: CoerceUnsized<S::Handle<U>>,
{
}

impl<T, S> AsRef<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T, S> AsMut<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn as_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T, S> Borrow<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn borrow(&self) -> &T {
        &*self
    }
}

impl<T, S> BorrowMut<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn borrow_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T, S> Deref for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Handle is guaranteed valid by internal invariant
        unsafe { self.storage.get(self.handle).as_ref() }
    }
}

impl<T, S> DerefMut for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Handle is guaranteed valid by internal invariant
        unsafe { self.storage.get(self.handle).as_mut() }
    }
}

impl<T, S> Drop for Box<T, S>
where
    T: ?Sized + Pointee,
    S: SingleElementStorage,
{
    fn drop(&mut self) {
        // SAFETY: Handle is guaranteed valid by internal invariant
        unsafe { self.storage.drop_single(self.handle) };
        // SAFETY: This is `drop`, so we know we're the last observer
        unsafe { ManuallyDrop::drop(&mut self.storage) };
    }
}

impl<T, S> Clone for Box<T, S>
where
    T: Pointee + Clone,
    S: SingleElementStorage + Default,
{
    fn clone(&self) -> Box<T, S> {
        let new_item = T::clone(&*self);
        Box::new(new_item)
    }
}

impl<T, S> Default for Box<T, S>
where
    T: Pointee + Default,
    S: SingleElementStorage + Default,
{
    fn default() -> Box<T, S> {
        Box::new(T::default())
    }
}

impl<T, S> PartialEq for Box<T, S>
where
    T: ?Sized + Pointee + PartialEq,
    S: SingleElementStorage,
{
    fn eq(&self, other: &Self) -> bool {
        T::eq(&*self, &*other)
    }
}

impl<T, S> Eq for Box<T, S>
where
    T: ?Sized + Pointee + Eq,
    S: SingleElementStorage,
{
}

impl<T, S> PartialOrd for Box<T, S>
where
    T: ?Sized + Pointee + PartialOrd,
    S: SingleElementStorage,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        T::partial_cmp(&*self, &*other)
    }
}

impl<T, S> Ord for Box<T, S>
where
    T: ?Sized + Pointee + Ord,
    S: SingleElementStorage,
{
    fn cmp(&self, other: &Self) -> Ordering {
        T::cmp(&*self, &*other)
    }
}

#[cfg(test)]
mod tests {
    use crate::inline::SingleElement;

    type Box<T> = super::Box<T, SingleElement<[usize; 4]>>;

    #[test]
    fn new() {
        let b = Box::new(1);
        assert_eq!(*b, 1);
    }

    #[test]
    fn new_in() {
        let b = Box::new_in(1, SingleElement::new());
        assert_eq!(*b, 1);
    }

    #[test]
    fn try_in() {
        let b = Box::new([1, 2]);
        let b2 = b
            .try_in::<SingleElement<[usize; 2]>>(SingleElement::new())
            .unwrap();

        assert_eq!(*b2, [1, 2]);

        let b3 = b2
            .try_in::<SingleElement<[u32; 1]>>(SingleElement::new())
            .unwrap_err();

        assert_eq!(*b3.0, [1, 2]);
    }
}
