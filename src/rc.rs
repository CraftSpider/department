//! A storage-based implementation of [`std::rc`]

use crate::base::{ClonesafeStorage, Storage};
use core::borrow::Borrow;
use core::cell::Cell;
use core::marker::PhantomData;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
#[cfg(feature = "unsize")]
use core::mem;
#[cfg(feature = "unsize")]
use core::ops::CoerceUnsized;
use core::ops::Deref;

#[repr(C)]
#[derive(Debug)]
struct RcBox<T: ?Sized> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: T,
}

impl<T: ?Sized> RcBox<T> {
    fn strong(&self) -> usize {
        self.strong.get()
    }

    fn inc_strong(&self) {
        let strong = self.strong.get();
        self.strong.set(strong + 1);
    }

    fn dec_strong(&self) {
        let strong = self.strong.get();
        self.strong.set(strong - 1);
    }

    fn weak(&self) -> usize {
        self.weak.get()
    }

    fn inc_weak(&self) {
        let weak = self.weak.get();
        self.weak.set(weak + 1);
    }

    fn dec_weak(&self) {
        let weak = self.weak.get();
        self.weak.set(weak - 1);
    }
}

impl<T> RcBox<T> {
    fn new(value: T) -> RcBox<T> {
        RcBox {
            strong: Cell::new(1),
            weak: Cell::new(1),
            value,
        }
    }
}

#[cfg(feature = "unsize")]
impl<T, U> CoerceUnsized<RcBox<U>> for RcBox<T>
where
    T: ?Sized + CoerceUnsized<U>,
    U: ?Sized,
{
}

/// Storage-based implementation of [`Rc`](std::rc::Rc).
///
/// Requires that the storage be a [`ClonesafeStorage`], which excludes inline and some other forms
/// of storage.
pub struct Rc<T: ?Sized, S: Storage + ClonesafeStorage> {
    handle: S::Handle<RcBox<T>>,
    storage: S,
    phantom: PhantomData<*mut ()>,
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Rc<T, S> {
    fn inner(&self) -> &RcBox<T> {
        // SAFETY: While Rc lives the inner handle is valid
        unsafe { self.storage.get(self.handle).as_ref() }
    }

    unsafe fn from_inner(handle: S::Handle<RcBox<T>>, storage: S) -> Rc<T, S> {
        Rc {
            handle,
            storage,
            phantom: PhantomData,
        }
    }

    /// Get a [`Weak`] from this [`Rc`]
    pub fn downgrade(this: &Self) -> Weak<T, S> {
        this.inner().inc_weak();
        Weak {
            handle: this.handle,
            storage: this.storage.clone(),
        }
    }

    /// Perform an unsizing operation on `self`. A temporary solution to limitations with
    /// manual unsizing.
    #[cfg(feature = "unsize")]
    pub fn coerce<U: ?Sized>(self) -> Rc<U, S>
    where
        T: Unsize<U>,
    {
        // SAFETY: Our handle is guaranteed valid by internal invariant
        let handle = unsafe { self.storage.coerce::<_, RcBox<U>>(self.handle) };
        let storage = self.storage.clone();
        // Ensure we don't decrement refcount
        mem::forget(self);
        Rc {
            handle,
            storage,
            phantom: PhantomData,
        }
    }
}

impl<T, S: Storage + ClonesafeStorage> Rc<T, S> {
    /// Create a new [`Rc`] from the provided value in some existing storage
    ///
    /// # Panics
    ///
    /// If the storage fails to allocate enough space for the provided type and associated
    /// information
    pub fn new_in(value: T, mut storage: S) -> Rc<T, S> {
        let handle = storage
            .create_single(RcBox::new(value))
            .unwrap_or_else(|_| panic!("Couldn't allocate RcBox"));
        unsafe { Self::from_inner(handle, storage) }
    }
}

impl<T, S: Storage + ClonesafeStorage + Default> Rc<T, S> {
    /// Create a new [`Rc`] from the provided value
    pub fn new(value: T) -> Rc<T, S> {
        Self::new_in(value, S::default())
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Drop for Rc<T, S> {
    fn drop(&mut self) {
        self.inner().dec_strong();
        if self.inner().strong() == 0 {
            unsafe { core::ptr::drop_in_place(&mut self.storage.get(self.handle).as_mut().value) };

            self.inner().dec_weak();

            if self.inner().weak() == 0 {
                unsafe { self.storage.deallocate_single(self.handle) }
            }
        }
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Clone for Rc<T, S> {
    fn clone(&self) -> Self {
        unsafe {
            self.inner().inc_strong();
            Self::from_inner(self.handle, self.storage.clone())
        }
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Deref for Rc<T, S> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> AsRef<T> for Rc<T, S> {
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Borrow<T> for Rc<T, S> {
    fn borrow(&self) -> &T {
        &**self
    }
}

#[cfg(feature = "unsize")]
impl<T, U, S> CoerceUnsized<Rc<U, S>> for Rc<T, S>
where
    T: ?Sized,
    U: ?Sized,
    S: Storage + ClonesafeStorage,
    S::Handle<RcBox<T>>: CoerceUnsized<S::Handle<RcBox<U>>>,
{
}

struct WeakInner<'a> {
    strong: &'a Cell<usize>,
    weak: &'a Cell<usize>,
}

impl WeakInner<'_> {
    fn strong(&self) -> usize {
        self.strong.get()
    }

    fn inc_strong(&self) {
        let strong = self.strong.get();
        self.strong.set(strong + 1);
    }

    fn weak(&self) -> usize {
        self.weak.get()
    }

    fn dec_weak(&self) {
        let weak = self.weak.get();
        self.weak.set(weak - 1);
    }
}

/// Storage-based implementation of [`std::rc::Weak`]
pub struct Weak<T: ?Sized, S: Storage + ClonesafeStorage> {
    handle: S::Handle<RcBox<T>>,
    storage: S,
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Weak<T, S> {
    fn inner(&self) -> Option<WeakInner<'_>> {
        Some(unsafe {
            let ptr = self.storage.get(self.handle).as_ptr();
            WeakInner {
                strong: &(*ptr).strong,
                weak: &(*ptr).weak,
            }
        })
    }

    /// Attempt to convert this [`Weak`] back into an [`Rc`]. Returns `None` if all strong
    /// references to the data have already been dropped.
    pub fn upgrade(&self) -> Option<Rc<T, S>> {
        let inner = self.inner()?;
        if inner.strong() == 0 {
            None
        } else {
            unsafe {
                inner.inc_strong();
                Some(Rc::from_inner(self.handle, self.storage.clone()))
            }
        }
    }
}

impl<T: ?Sized, S: Storage + ClonesafeStorage> Drop for Weak<T, S> {
    fn drop(&mut self) {
        let inner = if let Some(inner) = self.inner() {
            inner
        } else {
            return;
        };

        inner.dec_weak();
        if inner.weak() == 0 {
            unsafe { self.storage.deallocate_single(self.handle) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::VirtHeap;

    #[test]
    fn test_rc() {
        let heap: VirtHeap<u64, 16> = VirtHeap::new();

        let rc1 = Rc::new_in(1, &heap);
        let rc2 = Rc::clone(&rc1);
        let weak1 = Rc::downgrade(&rc2);

        assert_eq!(*rc1, 1);
        assert_eq!(*rc2, 1);

        let rc3 = weak1.upgrade().unwrap();

        assert_eq!(*rc3, 1);

        drop(rc1);
        drop(rc2);
        drop(rc3);

        assert!(matches!(weak1.upgrade(), None));
    }
}
