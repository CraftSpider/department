//! Common handle implementations, used across multiple storages
//!
//! These attempt to provide relevant 'pointer-like' interfaces, such as casting and coercion,
//! though not all handles may implement all items.

use core::fmt;
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::num::NonZeroUsize;
use core::ptr;
use core::ptr::{NonNull, Pointee};

/// Abstraction over common handle operations on a handle with type `T`
///
/// The fact that this supplies casting and metadata-retrieval slightly limits handles - they must
/// either be able to dereference themselves, or must hold the metadata for a type inline. On the
/// other hand, even our most restrictive storage does it this way, and implementing always-thin
/// handles, while theoretically possible, is deemed not worth the limitations. It is also possible
/// to implement such always-thin items via a custom `ThinBox` style type, meaning the lack of them
/// in this API does not prevent storages from having them entirely. This decision is open to change
/// before the release of 1.0 - feel free to open an issue if you have a compelling use-case.
pub trait Handle {
    /// The type of address for this handle. This is only [`PartialOrd`] instead of [`Ord`] because
    /// some handles may not be strictly greater or lesser than others (See [`FallbackStorage`](crate::fallback::FallbackStorage))
    type Addr: Copy + Eq + PartialOrd;

    /// The target type for this handle.
    type Target: ?Sized + Pointee;

    /// The type of this handle, with a different type in place of `T`
    type This<U: ?Sized>: Handle<Target = U>;

    /// Create this handle from an empty tuple handle and a metadata
    fn from_raw_parts(handle: Self::This<()>, meta: <Self::Target as Pointee>::Metadata) -> Self;

    /// Address of this handle. The exact meaning of 'address' may vary between handles, handles
    /// to different items may have the same address
    fn addr(self) -> Self::Addr;

    /// Metadata of `T` associated with this handle
    fn metadata(self) -> <Self::Target as Pointee>::Metadata;

    /// Convert this handle into one pointing to type `U`, discarding metadata
    fn cast<U>(self) -> Self::This<U>;

    /// Convert this handle into one pointing to type `U`, preserving metadata
    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = <Self::Target as Pointee>::Metadata>;

    /// Coerce this handle into an unsized type its current type. This is equivalent to invoking
    /// `CoerceUnsized` via an `as` cast, if it's implemented.
    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        Self::Target: Unsize<U>;
}

impl<T: ?Sized + Pointee> Handle for *const T {
    type Addr = usize;
    type Target = T;

    type This<U: ?Sized> = *const U;

    fn from_raw_parts(handle: Self::This<()>, meta: T::Metadata) -> Self {
        ptr::from_raw_parts(handle, meta)
    }

    fn addr(self) -> usize {
        self.cast::<()>() as usize
    }

    fn metadata(self) -> T::Metadata {
        ptr::metadata(self)
    }

    fn cast<U>(self) -> Self::This<U> {
        self.cast()
    }

    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = T::Metadata>,
    {
        let meta = ptr::metadata(self);
        ptr::from_raw_parts(self.cast(), meta)
    }

    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        T: Unsize<U>,
    {
        self as *const U
    }
}

impl<T: ?Sized + Pointee> Handle for *mut T {
    type Addr = usize;
    type Target = T;

    type This<U: ?Sized> = *mut U;

    fn from_raw_parts(handle: Self::This<()>, meta: T::Metadata) -> Self {
        ptr::from_raw_parts_mut(handle, meta)
    }

    fn addr(self) -> usize {
        self.cast::<()>() as usize
    }

    fn metadata(self) -> T::Metadata {
        ptr::metadata(self)
    }

    fn cast<U>(self) -> Self::This<U> {
        self.cast()
    }

    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = T::Metadata>,
    {
        let meta = ptr::metadata(self);
        ptr::from_raw_parts_mut(self.cast(), meta)
    }

    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        T: Unsize<U>,
    {
        self as *mut U
    }
}

impl<T: ?Sized + Pointee> Handle for NonNull<T> {
    type Addr = usize;
    type Target = T;

    type This<U: ?Sized> = NonNull<U>;

    fn from_raw_parts(handle: Self::This<()>, meta: T::Metadata) -> Self {
        NonNull::from_raw_parts(handle, meta)
    }

    fn addr(self) -> usize {
        self.cast::<()>().as_ptr() as usize
    }

    fn metadata(self) -> T::Metadata {
        ptr::metadata(self.as_ptr())
    }

    fn cast<U>(self) -> Self::This<U> {
        self.cast()
    }

    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = T::Metadata>,
    {
        let meta = ptr::metadata(self.as_ptr());
        NonNull::from_raw_parts(self.cast(), meta)
    }

    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        T: Unsize<U>,
    {
        self as NonNull<U>
    }
}

// FIXME: Replace with JustMetadata when that merges

/// A handle containing only metadata, all information about an items location is handled by
/// the storage
pub struct MetaHandle<T: ?Sized + Pointee>(T::Metadata);

impl<T: ?Sized + Pointee> MetaHandle<T> {
    /// Create a new instance of this handle from metadata for a type
    #[inline]
    pub const fn from_metadata(meta: T::Metadata) -> MetaHandle<T> {
        MetaHandle(meta)
    }

    /// Get the metadata contained within this handle
    #[inline]
    pub const fn metadata(self) -> T::Metadata {
        self.0
    }

    /// Cast this handle to any sized type, similar to [`NonNull::cast`][core::ptr::NonNull]
    #[inline]
    pub const fn cast<U>(self) -> MetaHandle<U> {
        MetaHandle::from_metadata(())
    }

    /// Cast this handle to any unsized type with the same metadata as it currently holds
    #[inline]
    pub const fn cast_unsized<U>(self) -> MetaHandle<U>
    where
        T: Pointee<Metadata = <U as Pointee>::Metadata>,
        U: ?Sized,
    {
        MetaHandle::from_metadata(self.metadata())
    }

    /// Coerce this handle to a type which unsizes from the current type
    #[cfg(feature = "unsize")]
    pub const fn coerce<U: ?Sized>(self) -> MetaHandle<U>
    where
        T: Unsize<U>,
    {
        let ptr: *const T = ptr::from_raw_parts(ptr::null(), self.metadata());
        let meta = ptr::metadata(ptr as *const U);
        MetaHandle::from_metadata(meta)
    }
}

impl<T: ?Sized + Pointee> Handle for MetaHandle<T> {
    type Addr = ();
    type Target = T;

    type This<U: ?Sized> = MetaHandle<U>;

    fn from_raw_parts(_: Self::This<()>, meta: T::Metadata) -> Self {
        MetaHandle::from_metadata(meta)
    }

    fn addr(self) {}

    fn metadata(self) -> T::Metadata {
        MetaHandle::metadata(self)
    }

    fn cast<U>(self) -> Self::This<U> {
        MetaHandle::from_metadata(())
    }

    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = T::Metadata>,
    {
        MetaHandle::from_metadata(self.metadata())
    }

    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        T: Unsize<U>,
    {
        let ptr = ptr::from_raw_parts::<T>(ptr::null(), self.metadata()) as *const U;
        let meta = ptr::metadata(ptr);
        MetaHandle::from_metadata(meta)
    }
}

impl<T: ?Sized> Copy for MetaHandle<T> {}
impl<T: ?Sized> Clone for MetaHandle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> PartialEq for MetaHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> fmt::Debug for MetaHandle<T>
where
    T: ?Sized + Pointee,
    T::Metadata: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MetaHandle").field(&self.0).finish()
    }
}

/// A handle containing an offset and some metadata, similar to a pointer but with the offset being
/// storage-specific instead of an address space. This handle reserves the offset [`usize::MAX`]
/// to allow niche-optimization.
pub struct OffsetMetaHandle<T: ?Sized + Pointee>(NonZeroUsize, T::Metadata);

impl<T: ?Sized + Pointee> OffsetMetaHandle<T> {
    /// Create a new instance of this handle from an offset and metadata for the type
    ///
    /// # Panics
    ///
    /// If offset is equal to `usize::MAX`, due to that being a reserved value
    #[inline]
    pub const fn from_offset_meta(offset: usize, meta: T::Metadata) -> OffsetMetaHandle<T> {
        assert!(offset != usize::MAX, "OffsetMetaHandle reserves usize::MAX for niche optimization");
        // SAFETY: We do `offset + 1`, and offset is not usize::MAX, so the resulting value will
        //         always be in-bounds and non-zero
        OffsetMetaHandle(unsafe { NonZeroUsize::new_unchecked(offset + 1) }, meta)
    }

    /// Get the offset of this handle
    #[inline]
    pub const fn offset(self) -> usize {
        self.0.get() - 1
    }

    /// Get the metadata contained within this handle
    #[inline]
    pub const fn metadata(self) -> T::Metadata {
        self.1
    }

    /// Add some usize to the offset. The user must ensure the resulting handle is valid
    #[inline]
    pub const fn add(self, offset: usize) -> OffsetMetaHandle<T> {
        OffsetMetaHandle::from_offset_meta(self.offset() + offset, self.metadata())
    }

    /// Subtract some usize from the offset. The user must ensure the resulting handle is valid
    #[inline]
    pub const fn sub(self, offset: usize) -> OffsetMetaHandle<T> {
        OffsetMetaHandle::from_offset_meta(self.offset() - offset, self.metadata())
    }

    /// Change the offset of this handle by some value. The user must ensure the resulting handle is
    /// valid
    pub const fn offset_by(self, offset: isize) -> OffsetMetaHandle<T> {
        self.sub(offset.unsigned_abs())
    }

    /// Cast this handle to any sized type, similar to [`NonNull::cast`][core::ptr::NonNull]
    #[inline]
    pub const fn cast<U>(self) -> OffsetMetaHandle<U> {
        OffsetMetaHandle::from_offset_meta(self.offset(), ())
    }

    /// Cast this handle to any unsized type with the same metadata as it currently holds
    #[inline]
    pub const fn cast_unsized<U>(self) -> OffsetMetaHandle<U>
    where
        T: Pointee<Metadata = <U as Pointee>::Metadata>,
        U: ?Sized,
    {
        OffsetMetaHandle::from_offset_meta(self.offset(), self.metadata())
    }

    /// Coerce this handle to a type which unsizes from the current type
    #[cfg(feature = "unsize")]
    pub const fn coerce<U: ?Sized>(self) -> OffsetMetaHandle<U>
    where
        T: Unsize<U>,
    {
        let ptr: *const T = ptr::from_raw_parts(ptr::null(), self.metadata());
        let meta = ptr::metadata(ptr as *const U);
        OffsetMetaHandle(self.0, meta)
    }
}

impl<T: ?Sized + Pointee> Handle for OffsetMetaHandle<T> {
    type Addr = usize;
    type Target = T;

    type This<U: ?Sized> = OffsetMetaHandle<U>;

    fn from_raw_parts(handle: Self::This<()>, meta: T::Metadata) -> Self {
        OffsetMetaHandle::from_offset_meta(handle.offset(), meta)
    }

    fn addr(self) -> usize {
        self.offset()
    }

    fn metadata(self) -> T::Metadata {
        OffsetMetaHandle::metadata(self)
    }

    fn cast<U>(self) -> Self::This<U> {
        OffsetMetaHandle::from_offset_meta(self.offset(), ())
    }

    fn cast_unsized<U>(self) -> Self::This<U>
    where
        U: ?Sized + Pointee<Metadata = T::Metadata>,
    {
        OffsetMetaHandle::from_offset_meta(self.offset(), self.metadata())
    }

    #[cfg(feature = "unsize")]
    fn coerce<U: ?Sized>(self) -> Self::This<U>
    where
        T: Unsize<U>,
    {
        let ptr = ptr::from_raw_parts::<T>(ptr::null(), self.metadata()) as *const U;
        let meta = ptr::metadata(ptr);
        OffsetMetaHandle::from_offset_meta(self.offset(), meta)
    }
}

impl<T: ?Sized> Copy for OffsetMetaHandle<T> {}
impl<T: ?Sized> Clone for OffsetMetaHandle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> PartialEq for OffsetMetaHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T> fmt::Debug for OffsetMetaHandle<T>
where
    T: ?Sized + Pointee,
    T::Metadata: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OffsetMetaHandle")
            .field("offset", &self.offset())
            .field("metadata", &self.metadata())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_handle() {
        let h1 = MetaHandle::<str>::from_metadata(1);
        assert_eq!(h1.metadata(), 1);
        let h2 = h1.cast::<()>();
        assert_eq!(h2.metadata(), ());
        let h3 = h1.cast_unsized::<[u8]>();
        assert_eq!(h3.metadata(), 1);

        assert_eq!(h1, MetaHandle::from_raw_parts(h2, 1));
        assert_eq!(h3, MetaHandle::from_raw_parts(h2, 1));
    }
}
