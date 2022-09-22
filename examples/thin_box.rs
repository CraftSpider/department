#![feature(unsize, ptr_metadata)]
#![feature(layout_for_ptr)]

use department::alloc::GlobalAlloc;
use department::backing::{Align4, Align8, Backing};
use department::base::Storage;
use department::handles::{Handle, MetaHandle};
use department::inline::SingleInline;
use spin::RwLock;
use std::alloc::Layout;
use std::fmt::{Debug, Formatter};
use std::marker::{PhantomData, Unsize};
use std::ops::Deref;
use std::ptr::{NonNull, Pointee};
use std::{mem, ptr};

pub(crate) fn layout_of<T: ?Sized + Pointee>(meta: T::Metadata) -> Layout {
    let pointer = ptr::from_raw_parts(ptr::null(), meta);
    // SAFETY: The provided metadata is passed by value, and thus must be a valid instance of the
    //         metadata for `T`
    unsafe { Layout::for_value_raw::<T>(pointer) }
}

#[repr(C)]
pub struct ThinInner<M, T: ?Sized> {
    meta: M,
    value: T,
}

pub struct ThinBox<T: ?Sized, S: Storage = GlobalAlloc> {
    handle: S::Handle<()>,
    storage: S,
    phantom: PhantomData<T>,
}

impl<T: ?Sized, S: Storage> ThinBox<T, S> {
    fn metadata(&self) -> <T as Pointee>::Metadata {
        let handle = S::cast::<_, <T as Pointee>::Metadata>(self.handle);
        unsafe { *self.storage.get(handle).as_ref() }
    }
}

impl<T: ?Sized, S: Storage + Default> ThinBox<T, S> {
    pub fn unsize_new<U: Unsize<T>>(value: U) -> Self {
        let meta = ptr::metadata::<T>(&value as &T);

        let mut storage = S::default();

        let handle = storage
            .allocate_single::<ThinInner<<T as Pointee>::Metadata, U>>(())
            .unwrap();

        let inner_ptr: NonNull<ThinInner<<T as Pointee>::Metadata, U>> =
            unsafe { storage.get(handle) };

        unsafe {
            (*inner_ptr.as_ptr()).meta = meta;
            (*inner_ptr.as_ptr()).value = value;
        }

        ThinBox {
            handle: S::cast(handle),
            storage,
            phantom: PhantomData,
        }
    }
}

impl<T, S: Storage + Default> ThinBox<T, S> {
    pub fn new(value: T) -> Self {
        let mut storage = S::default();
        let handle = storage.allocate_single::<ThinInner<(), T>>(()).unwrap();
        let ptr = unsafe { storage.get(handle) };
        unsafe { (*ptr.as_ptr()).value = value };
        ThinBox {
            handle: S::cast(handle),
            storage,
            phantom: PhantomData,
        }
    }
}

impl<T: ?Sized + Debug, S: Storage> Debug for ThinBox<T, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized, S: Storage> Deref for ThinBox<T, S> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let handle = S::cast::<_, <T as Pointee>::Metadata>(self.handle);
        let ptr = unsafe { self.storage.get(handle) };
        let meta = unsafe { *ptr.as_ptr() };
        let data_ptr = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(1)) };
        unsafe { NonNull::from_raw_parts(data_ptr.cast(), meta).as_ref() }
    }
}

impl<T: ?Sized, S: Storage> Drop for ThinBox<T, S> {
    fn drop(&mut self) {
        union MetaCaster<T: ?Sized> {
            meta: <T as Pointee>::Metadata,
            meta2: <ThinInner<<T as Pointee>::Metadata, T> as Pointee>::Metadata,
        }

        let meta = unsafe {
            MetaCaster::<T> {
                meta: self.metadata(),
            }
            .meta2
        };
        let handle = S::from_raw_parts::<ThinInner<<T as Pointee>::Metadata, T>>(self.handle, meta);
        unsafe { self.storage.drop_single(handle) }
    }
}

fn sized_item() {
    dbg!(ThinBox::<_, GlobalAlloc>::new(1));
    dbg!(ThinBox::<_, SingleInline<Backing<16, Align4>>>::new(1));
}

fn thin_range() {
    dbg!(ThinBox::<[i32], GlobalAlloc>::unsize_new([1, 2, 3]));
    dbg!(ThinBox::<[i32], SingleInline<Backing<24, Align8>>>::unsize_new([1, 2, 3]));
}

fn thin_dyn() {
    dbg!(ThinBox::<dyn Debug, GlobalAlloc>::unsize_new(
        "Hello World!"
    ));
    dbg!(ThinBox::<dyn Debug, SingleInline<Backing<24, Align8>>>::unsize_new("Hello World!"));
}

fn ultra_thin() {
    struct SyncPtr(NonNull<()>);

    unsafe impl Send for SyncPtr {}
    unsafe impl Sync for SyncPtr {}

    static THIN_BACKING: RwLock<SyncPtr> = RwLock::new(SyncPtr(NonNull::dangling()));

    // This is incredibly likely to cause incorrectness or unsoundness - this implementation is just
    // here to show off the idea of a ZST box.
    #[derive(Default)]
    struct ThinStorage;

    unsafe impl Storage for ThinStorage {
        type Handle<T: ?Sized> = MetaHandle<T>;

        unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
            NonNull::from_raw_parts(THIN_BACKING.read().0, handle.metadata())
        }

        fn from_raw_parts<T: ?Sized + Pointee>(
            handle: Self::Handle<()>,
            meta: T::Metadata,
        ) -> Self::Handle<T> {
            MetaHandle::from_raw_parts(handle, meta)
        }

        fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U> {
            MetaHandle::cast(handle)
        }

        fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
            handle: Self::Handle<T>,
        ) -> Self::Handle<U> {
            MetaHandle::cast_unsized(handle)
        }

        fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
            handle: Self::Handle<T>,
        ) -> Self::Handle<U> {
            MetaHandle::coerce(handle)
        }

        fn allocate_single<T: ?Sized + Pointee>(
            &mut self,
            meta: T::Metadata,
        ) -> department::error::Result<Self::Handle<T>> {
            let layout = layout_of::<T>(meta);
            THIN_BACKING.write().0 =
                NonNull::new(unsafe { std::alloc::alloc(layout).cast() }).unwrap();
            Ok(MetaHandle::from_metadata(meta))
        }

        unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
            let layout = layout_of::<T>(handle.metadata());
            let mut backing = THIN_BACKING.write();
            std::alloc::dealloc(backing.0.as_ptr().cast(), layout);
            backing.0 = NonNull::dangling();
        }
    }

    let b = ThinBox::<[i32], ThinStorage>::unsize_new([1, 2, 3, 4]);

    assert_eq!(&*b, &[1, 2, 3, 4]);
    assert_eq!(mem::size_of::<ThinBox<[i32], ThinStorage>>(), 0);
}

fn main() {
    sized_item();
    thin_range();
    thin_dyn();
    ultra_thin();
}
