use crate::base::{ClonesafeStorage, Storage};
use core::borrow::Borrow;
use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::Deref;

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

    pub fn downgrade(this: &Self) -> Weak<T, S> {
        this.inner().inc_weak();
        Weak {
            handle: this.handle,
            storage: this.storage.clone(),
        }
    }
}

impl<T, S: Storage + ClonesafeStorage> Rc<T, S> {
    pub fn new_in(value: T, mut storage: S) -> Rc<T, S> {
        let handle = storage
            .create_single(RcBox::new(value))
            .unwrap_or_else(|_| panic!("Couldn't allocate RcBox"));
        unsafe { Self::from_inner(handle, storage) }
    }
}

impl<T, S: Storage + ClonesafeStorage + Default> Rc<T, S> {
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

pub struct WeakInner<'a> {
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
