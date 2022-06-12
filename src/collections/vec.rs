use core::borrow::{Borrow, BorrowMut};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut, Index, IndexMut};
use core::{fmt, mem, ptr, slice};

use crate::base::Storage;
use crate::error::Result;

/// Storage based implementation of [`Vec`](`std::vec::Vec`)
pub struct Vec<T, S>
where
    S: Storage,
{
    handle: S::Handle<[MaybeUninit<T>]>,
    len: usize,
    storage: S,
}

impl<T, S> Vec<T, S>
where
    S: Storage + Default,
{
    /// Create a new, empty [`Vec`], creating a default instance of the desired storage.
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn new() -> Vec<T, S> {
        let mut storage = S::default();

        Vec {
            handle: storage.allocate_single(0).unwrap(),
            len: 0,
            storage,
        }
    }

    /// Attempt to create a new, empty [`Vec`], creating a default instance of the desired storage.
    pub fn try_new() -> Result<Vec<T, S>> {
        let mut storage = S::default();

        Ok(Vec {
            handle: storage.allocate_single(0)?,
            len: 0,
            storage,
        })
    }

    /// Create a new [`Vec`], with a pre-allocated capacity equal to `size`.
    /// Uses a new default instance of the desired storage.
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn with_capacity(size: usize) -> Vec<T, S> {
        let mut storage = S::default();

        Vec {
            handle: storage.allocate_single(size).unwrap(),
            len: 0,
            storage,
        }
    }
}

impl<T, S> Vec<T, S>
where
    S: Storage,
{
    /// Create a new, empty [`Vec`], using the provided storage instance.
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn new_in(mut storage: S) -> Vec<T, S> {
        Vec {
            handle: storage.allocate_single(0).unwrap(),
            len: 0,
            storage,
        }
    }

    /// Attempt to create a new, empty [`Vec`], using the provided storage instance.
    pub fn try_new_in(mut storage: S) -> Result<Vec<T, S>> {
        Ok(Vec {
            handle: storage.allocate_single(0)?,
            len: 0,
            storage,
        })
    }

    /// Create a new [`Vec`], with a pre-allocated capacity equal to `size`.
    /// Uses the provided instance of the desired storage.
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn with_capacity_in(size: usize, mut storage: S) -> Vec<T, S> {
        Vec {
            handle: storage.allocate_single(size).unwrap(),
            len: 0,
            storage,
        }
    }

    /// Check if the vector contains no element
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the current length of the vector
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check the vector's current capacity, the maximum length it can grow to without reallocating
    pub fn capacity(&self) -> usize {
        // SAFETY: Handle is guaranteed valid by internal invariant
        let ptr = unsafe { self.storage.get(self.handle) };
        // SAFETY: Valid handles are guaranteed to return a valid pointer
        unsafe { ptr.as_ref().len() }
    }

    /// Add a new element onto the end of the vector
    pub fn push(&mut self, val: T) {
        let old_capacity = self.capacity();

        if self.len + 1 > old_capacity {
            let new_capacity = if old_capacity == 0 {
                2
            } else {
                old_capacity * 2
            };

            // SAFETY: Handle is guaranteed valid by internal invariant
            //         New capacity cannot be less than old due to how it's calculated
            unsafe {
                self.handle = self
                    .storage
                    .try_grow(self.handle, new_capacity)
                    .expect("Couldn't grow Vec buffer");
            }
        }

        // SAFETY: Handle is guaranteed valid by internal invariant
        let mut ptr = unsafe { self.storage.get(self.handle) };
        // SAFETY: Valid handles are guaranteed to return valid pointers
        unsafe { ptr.as_mut()[self.len] = MaybeUninit::new(val) };
        self.len += 1;
    }

    /// Remove the element at the end of the vector and return it
    pub fn pop(&mut self) -> T {
        self.len -= 1;

        // SAFETY: Handle is guaranteed valid by internal invariant
        let mut ptr = unsafe { self.storage.get(self.handle) };
        // SAFETY: Valid handles are guaranteed to return valid pointers
        let item = unsafe { &mut ptr.as_mut()[self.len] };
        let out = mem::replace(item, MaybeUninit::uninit());
        // SAFETY: Popped element must be initialized, as length counts initialized items
        unsafe { out.assume_init() }
    }

    /// Get an iterator over this vector
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.as_ref().iter()
    }

    /// Get a mutable iterator over this vector
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.as_mut().iter_mut()
    }
}

impl<T, S> fmt::Debug for Vec<T, S>
where
    T: fmt::Debug,
    S: Storage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ref())
    }
}

impl<T, S> Default for Vec<T, S>
where
    S: Storage + Default,
{
    fn default() -> Vec<T, S> {
        Vec::new()
    }
}

impl<T, S> AsRef<[T]> for Vec<T, S>
where
    S: Storage,
{
    fn as_ref(&self) -> &[T] {
        &*self
    }
}

impl<T, S> AsMut<[T]> for Vec<T, S>
where
    S: Storage,
{
    fn as_mut(&mut self) -> &mut [T] {
        &mut *self
    }
}

impl<T, S> Borrow<[T]> for Vec<T, S>
where
    S: Storage,
{
    fn borrow(&self) -> &[T] {
        &*self
    }
}

impl<T, S> BorrowMut<[T]> for Vec<T, S>
where
    S: Storage,
{
    fn borrow_mut(&mut self) -> &mut [T] {
        &mut *self
    }
}

impl<T, S> Deref for Vec<T, S>
where
    S: Storage,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        // SAFETY: Handle is guaranteed valid by internal invariant
        let ptr = unsafe { self.storage.get(self.handle) };
        // SAFETY: Valid handles are guaranteed to return valid pointers
        //         Length counts initialized items, safe to interpret as `T`
        unsafe { slice::from_raw_parts(ptr.cast().as_ptr(), self.len) }
    }
}

impl<T, S> DerefMut for Vec<T, S>
where
    S: Storage,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Handle is guaranteed valid by internal invariant
        let ptr = unsafe { self.storage.get(self.handle) };
        // SAFETY: Valid handles are guaranteed to return valid pointers
        //         Length counts initialized items, safe to interpret as `T`
        unsafe { slice::from_raw_parts_mut(ptr.cast().as_ptr(), self.len) }
    }
}

impl<T, S> Drop for Vec<T, S>
where
    S: Storage,
{
    fn drop(&mut self) {
        for i in self.as_mut() {
            // SAFETY: This is `drop`, so no one else will observe these values
            unsafe { ptr::drop_in_place(i) }
        }
        // SAFETY: Handle is guaranteed valid by internal invariant
        unsafe { self.storage.deallocate_single(self.handle) }
    }
}

impl<T, S> Index<usize> for Vec<T, S>
where
    S: Storage,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_ref()[index]
    }
}

impl<T, S> IndexMut<usize> for Vec<T, S>
where
    S: Storage,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut()[index]
    }
}

impl<T, S> Clone for Vec<T, S>
where
    T: Clone,
    S: Storage + Clone,
{
    fn clone(&self) -> Self {
        let mut new_storage = self.storage.clone();
        let new_handle = new_storage
            .allocate_single::<[MaybeUninit<T>]>(self.len())
            .expect("Couldn't allocate new array");

        // SAFETY: New handle is guaranteed valid as allocate succeeded
        let mut ptr = unsafe { new_storage.get(new_handle) };
        // SAFETY: Valid handles are guaranteed to return valid pointers
        let new_iter = unsafe { ptr.as_mut().iter_mut() };
        for (old, new) in self.as_ref().iter().zip(new_iter) {
            new.write(old.clone());
        }

        Vec {
            handle: new_handle,
            len: self.len(),
            storage: new_storage,
        }
    }
}

impl<T, S> From<&[T]> for Vec<T, S>
where
    T: Clone,
    S: Storage + Default,
{
    fn from(val: &[T]) -> Self {
        let mut v = Vec::with_capacity(val.len());
        v.extend(val.iter().cloned());
        v
    }
}

impl<T, S, const N: usize> From<[T; N]> for Vec<T, S>
where
    S: Storage + Default,
{
    fn from(val: [T; N]) -> Self {
        let mut v = Vec::with_capacity(N);
        v.extend(val);
        v
    }
}

impl<T, S> From<(&[T], S)> for Vec<T, S>
where
    T: Clone,
    S: Storage,
{
    fn from(val: (&[T], S)) -> Self {
        let mut v = Vec::with_capacity_in(val.0.len(), val.1);
        v.extend(val.0.iter().cloned());
        v
    }
}

impl<T, S, const N: usize> From<([T; N], S)> for Vec<T, S>
where
    S: Storage,
{
    fn from(val: ([T; N], S)) -> Self {
        let mut v = Vec::with_capacity_in(N, val.1);
        v.extend(val.0);
        v
    }
}

impl<T, S> Extend<T> for Vec<T, S>
where
    S: Storage,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(|i| self.push(i))
    }
}

#[cfg(test)]
mod tests {
    use crate::inline::SingleInline;

    type Vec<T> = super::Vec<T, SingleInline<[usize; 16]>>;

    #[test]
    fn vec_new() {
        let v = Vec::<u32>::new();
        assert_eq!(v.len(), 0);
        assert_eq!(v.as_ref(), &[]);
    }

    #[test]
    fn vec_push() {
        let mut v = Vec::<u32>::new();
        v.push(1);
        v.push(2);

        assert_eq!(v.len(), 2);
        assert_eq!(v.as_ref(), &[1, 2]);
    }

    #[test]
    fn vec_pop() {
        let mut v = Vec::<u32>::new();
        v.push(1);
        v.push(2);

        assert_eq!(v.pop(), 2);
        assert_eq!(v.pop(), 1);
        assert_eq!(v.len(), 0);
    }

    #[test]
    fn vec_clone() {
        let mut v = Vec::<u32>::new();
        v.push(1);
        v.push(2);

        let v2 = v.clone();

        assert_eq!(v2.as_ref(), &[1, 2]);
    }
}
