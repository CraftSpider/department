use core::fmt;
use core::mem::MaybeUninit;
use core::ops::{Index, IndexMut};

use crate::traits;
use crate::traits::SingleRangeStorage;

pub struct Vec<T, S>
where
    S: SingleRangeStorage,
{
    handle: S::Handle<T>,
    len: usize,
    storage: S,
}

impl<T, S> Vec<T, S>
where
    S: SingleRangeStorage + Default,
{
    pub fn new() -> Vec<T, S> {
        let mut storage = S::default();

        Vec {
            handle: storage.allocate_single(0).unwrap_or_else(|_| panic!()),
            len: 0,
            storage,
        }
    }

    pub fn try_new() -> traits::Result<Vec<T, S>> {
        let mut storage = S::default();

        Ok(Vec {
            handle: storage.allocate_single(0)?,
            len: 0,
            storage,
        })
    }
}

impl<T, S> Vec<T, S>
where
    S: SingleRangeStorage,
{
    pub fn new_in(mut storage: S) -> Vec<T, S> {
        Vec {
            handle: storage.allocate_single(0).unwrap_or_else(|_| panic!()),
            len: 0,
            storage,
        }
    }

    pub fn try_new_in(mut storage: S) -> traits::Result<Vec<T, S>> {
        Ok(Vec {
            handle: storage.allocate_single(0)?,
            len: 0,
            storage,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        unsafe { self.storage.get(self.handle).as_ref().len() }
    }

    pub fn push(&mut self, val: T) {
        let old_capacity = self.capacity();
        if self.len + 1 >= old_capacity {
            unsafe {
                self.storage
                    .try_grow(self.handle, old_capacity * 2)
                    .expect("Couldn't grow Vec buffer")
            };
        }

        let mut ptr = unsafe { self.storage.get(self.handle) };
        unsafe { ptr.as_mut()[self.len] = MaybeUninit::new(val) };
        self.len += 1;
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.as_ref().into_iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.as_mut().into_iter()
    }
}

impl<T, S> fmt::Debug for Vec<T, S>
where
    T: fmt::Debug,
    S: SingleRangeStorage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ref())
    }
}

impl<T, S> AsRef<[T]> for Vec<T, S>
where
    S: SingleRangeStorage,
{
    fn as_ref(&self) -> &[T] {
        let ptr = unsafe { self.storage.get(self.handle) };
        unsafe { std::slice::from_raw_parts(ptr.cast().as_ptr(), self.len) }
    }
}

impl<T, S> AsMut<[T]> for Vec<T, S>
where
    S: SingleRangeStorage,
{
    fn as_mut(&mut self) -> &mut [T] {
        let ptr = unsafe { self.storage.get(self.handle) };
        unsafe { std::slice::from_raw_parts_mut(ptr.cast().as_ptr(), self.len) }
    }
}

impl<T, S> Index<usize> for Vec<T, S>
where
    S: SingleRangeStorage,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!("Index out of range: Length {}, index {}", self.len, index);
        }

        // SAFETY: We know items up to len are init
        unsafe { self.storage.get(self.handle).as_ref()[index].assume_init_ref() }
    }
}

impl<T, S> IndexMut<usize> for Vec<T, S>
where
    S: SingleRangeStorage,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            panic!("Index out of range: Length {}, index {}", self.len, index);
        }

        unsafe { self.storage.get(self.handle).as_mut()[index].assume_init_mut() }
    }
}

impl<T, S> Drop for Vec<T, S>
where
    S: SingleRangeStorage,
{
    fn drop(&mut self) {
        unsafe { self.storage.deallocate_single(self.handle) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline::SingleRange;

    type Vec<T> = super::Vec<T, SingleRange<usize, 16>>;

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
}
