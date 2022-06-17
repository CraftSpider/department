//! A storage-based implementation of [`std::boxed`]

use core::alloc::Layout;
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::mem::ManuallyDrop;
#[cfg(feature = "unsize")]
use core::ops::CoerceUnsized;
use core::ops::{Deref, DerefMut};
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem, ptr};

use crate::base::{FromLeakedStorage, LeaksafeStorage, Storage};

/// Storage-based implementation of [`Box`](std::boxed::Box).
///
/// Note that unsizing coercion currently isn't expressive enough to support all storage types,
/// so the implementation provides a `coerce` method which can be used to emulate the same
/// functionality.
pub struct Box<T: ?Sized + Pointee, S: Storage> {
    handle: S::Handle<T>,
    storage: ManuallyDrop<S>,
}

impl<T, S> Box<T, S>
where
    T: Pointee,
    S: Storage + Default,
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
    S: Storage,
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
    S: Storage,
{
    /// Attempt to move the value from this `Box` into another one using a different backing
    /// storage. In case of failure, the original `Box` is returned unchanged.
    pub fn try_in<Ns>(mut self, mut new_storage: Ns) -> Result<Box<T, Ns>, (Box<T, S>, Ns)>
    where
        Ns: Storage,
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

        // SAFETY: We just created the new storage and handle
        Ok(unsafe { Box::from_parts(new_storage, new_handle) })
    }

    /// Consumes and leaks this box, returning a mutable reference.
    ///
    /// The returned data lives for the rest of the program's life, dropping the reference will
    /// cause a memory leak. If this isn't acceptable, use [`Box::from_raw`] or [`Box::from_raw_in`]
    /// to regain ownership of this memory.
    pub fn leak<'a>(this: Self) -> &'a mut T
    where
        S: 'a + LeaksafeStorage,
    {
        // SAFETY: `into_raw` is guaranteed to return a pointer valid for any `'a` less than the
        //         lifetime of the storage
        unsafe { Box::into_raw(this).as_mut() }
    }

    /// Consumes and leaks this box, returning a raw pointer.
    ///
    /// The returned data lives for the rest of the program's life, dropping the pointer will
    /// cause a memory leak. If this isn't acceptable, use [`Box::from_raw`] or [`Box::from_raw_in`]
    /// to regain ownership of this memory.
    pub fn into_raw(mut this: Self) -> NonNull<T>
    where
        S: LeaksafeStorage,
    {
        // SAFETY: Handle is valid by internal invariant
        let out = unsafe { this.storage.get(this.handle) };
        // SAFETY: We consume self, so no one will touch us after this
        unsafe {
            ManuallyDrop::drop(&mut this.storage);
        }
        mem::forget(this);
        out
    }

    /// Construct a box from a raw pointer. After calling this function, the provided pointer
    /// is owned by this [`Box`]. Dropping this box will run the storage destructor on the pointer.
    ///
    /// # Safety
    ///
    /// The provided pointer must be unleak-compatible for the default instance of the storage type.
    /// See [`FromLeakedStorage::unleak_ptr`] for the exact definition of unleak-compatible.
    pub unsafe fn from_raw(ptr: *mut T) -> Box<T, S>
    where
        S: FromLeakedStorage + Default,
    {
        let storage = S::default();
        // SAFETY: Our safety requirements allow this
        let handle = unsafe { storage.unleak_ptr(ptr) };
        // SAFETY: We just created this handle from the same storage as we're passing
        unsafe { Box::from_parts(storage, handle) }
    }

    /// Construct a box from a raw pointer. After calling this function, the provided pointer
    /// is owned by this [`Box`]. Dropping this box will run the storage destructor on the pointer.
    ///
    /// # Safety
    ///
    /// The provided pointer must be unleak-compatible for the provided instance of the storage
    /// type. See [`FromLeakedStorage::unleak_ptr`] for the exact definition of unleak-compatible.
    pub unsafe fn from_raw_in(ptr: *mut T, storage: S) -> Box<T, S>
    where
        S: FromLeakedStorage,
    {
        // SAFETY: Our safety requirements allow this
        let handle = unsafe { storage.unleak_ptr(ptr) };
        // SAFETY: We just created this handle from the same storage as we're passing
        unsafe { Box::from_parts(storage, handle) }
    }

    /// Convert this box into its component storage and handle
    pub fn into_parts(mut self) -> (S, S::Handle<T>) {
        // SAFETY: We consume self, so no one will touch us after this
        let storage = unsafe { ManuallyDrop::take(&mut self.storage) };
        let handle = self.handle;
        mem::forget(self);
        (storage, handle)
    }

    /// Create a box from a component storage and handle
    ///
    /// # Safety
    ///
    /// This method takes ownership of the provided handle, the storage must be valid to get or
    /// deallocate the handle.
    pub unsafe fn from_parts(storage: S, handle: S::Handle<T>) -> Box<T, S> {
        Box {
            handle,
            storage: ManuallyDrop::new(storage),
        }
    }

    /// Perform an unsizing operation on `self`. A temporary solution to limitations with
    /// manual unsizing.
    #[cfg(feature = "unsize")]
    pub fn coerce<U: ?Sized>(mut self) -> Box<U, S>
    where
        T: Unsize<U>,
    {
        let handle = S::coerce::<_, U>(self.handle);
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
    S: Storage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ref())
    }
}

impl<T, S> fmt::Display for Box<T, S>
where
    T: ?Sized + fmt::Display,
    S: Storage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[cfg(feature = "unsize")]
impl<T, U, S> CoerceUnsized<Box<U, S>> for Box<T, S>
where
    T: ?Sized + Pointee,
    U: ?Sized + Pointee,
    S: Storage,
    S::Handle<T>: CoerceUnsized<S::Handle<U>>,
{
}

impl<T, S> AsRef<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
{
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T, S> AsMut<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
{
    fn as_mut(&mut self) -> &mut T {
        &mut **self
    }
}

impl<T, S> Borrow<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
{
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T, S> BorrowMut<T> for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
{
    fn borrow_mut(&mut self) -> &mut T {
        &mut **self
    }
}

impl<T, S> Deref for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
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
    S: Storage,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Handle is guaranteed valid by internal invariant
        unsafe { self.storage.get(self.handle).as_mut() }
    }
}

impl<T, S> Drop for Box<T, S>
where
    T: ?Sized + Pointee,
    S: Storage,
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
    S: Storage + Default,
{
    fn clone(&self) -> Box<T, S> {
        let new_item = T::clone(&**self);
        Box::new(new_item)
    }
}

impl<T, S> Default for Box<T, S>
where
    T: Pointee + Default,
    S: Storage + Default,
{
    fn default() -> Box<T, S> {
        Box::new(T::default())
    }
}

impl<T, S> PartialEq for Box<T, S>
where
    T: ?Sized + Pointee + PartialEq,
    S: Storage,
{
    fn eq(&self, other: &Self) -> bool {
        T::eq(&**self, &**other)
    }
}

impl<T, S> Eq for Box<T, S>
where
    T: ?Sized + Pointee + Eq,
    S: Storage,
{
}

impl<T, S> PartialOrd for Box<T, S>
where
    T: ?Sized + Pointee + PartialOrd,
    S: Storage,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        T::partial_cmp(&**self, &**other)
    }
}

impl<T, S> Ord for Box<T, S>
where
    T: ?Sized + Pointee + Ord,
    S: Storage,
{
    fn cmp(&self, other: &Self) -> Ordering {
        T::cmp(&**self, &**other)
    }
}

#[cfg(test)]
mod tests {
    use crate::inline::SingleInline;

    type Box<T> = super::Box<T, SingleInline<[usize; 4]>>;

    #[test]
    fn new() {
        let b = Box::new(1);
        assert_eq!(*b, 1);
    }

    #[test]
    fn new_in() {
        let b = Box::new_in(1, SingleInline::new());
        assert_eq!(*b, 1);
    }

    #[test]
    fn try_in() {
        let b = Box::new([1, 2]);
        let b2 = b
            .try_in::<SingleInline<[usize; 2]>>(SingleInline::new())
            .unwrap();

        assert_eq!(*b2, [1, 2]);

        let b3 = b2
            .try_in::<SingleInline<[u32; 1]>>(SingleInline::new())
            .unwrap_err();

        assert_eq!(*b3.0, [1, 2]);
    }
}
