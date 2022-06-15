//! Common handle implementations, used across multiple storages
//!
//! These attempt to provide relevant 'pointer-like' interfaces, such as casting and coercion,
//! though not all handles may implement all items.

#[cfg(feature = "unsize")]
use core::marker::Unsize;
#[cfg(feature = "unsize")]
use core::ptr;
use core::ptr::Pointee;
use core::fmt;

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
        MetaHandle(())
    }

    /// Cast this handle to any unsized type with the same metadata as it currently holds
    #[inline]
    pub const fn cast_unsized<U>(self) -> MetaHandle<U>
        where
            T: Pointee<Metadata = <U as Pointee>::Metadata>,
            U: ?Sized,
    {
        MetaHandle(self.0)
    }

    /// Coerce this handle to a type which unsizes from the current type
    #[cfg(feature = "unsize")]
    pub const fn coerce<U: ?Sized>(self) -> MetaHandle<U>
    where
        T: Unsize<U>,
    {
        let ptr: *const T = ptr::from_raw_parts(ptr::null(), self.0);
        let meta = ptr::metadata(ptr as *const U);
        MetaHandle(meta)
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
        f.debug_tuple("MetaHandle")
            .field(&self.0)
            .finish()
    }
}

/// A handle containing an offset and some metadata, similar to a pointer but with the offset being
/// storage-specific instead of an address space
pub struct OffsetMetaHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized + Pointee> OffsetMetaHandle<T> {
    /// Create a new instance of this handle from an offset and metadata for the type
    #[inline]
    pub const fn from_offset_meta(offset: usize, meta: T::Metadata) -> OffsetMetaHandle<T> {
        OffsetMetaHandle(offset, meta)
    }

    /// Get the offset of this handle
    #[inline]
    pub const fn offset(self) -> usize {
        self.0
    }

    /// Get the metadata contained within this handle
    #[inline]
    pub const fn metadata(self) -> T::Metadata {
        self.1
    }

    /// Add some usize to the offset. The user must ensure the resulting handle is valid
    #[inline]
    pub const fn add(self, offset: usize) -> OffsetMetaHandle<T> {
        OffsetMetaHandle::from_offset_meta(self.0 + offset, self.1)
    }

    /// Subtract some usize from the offset. The user must ensure the resulting handle is valid
    #[inline]
    pub const fn sub(self, offset: usize) -> OffsetMetaHandle<T> {
        OffsetMetaHandle::from_offset_meta(self.0 - offset, self.1)
    }

    /// Change the offset of this handle by some value. The user must ensure the resulting handle is
    /// valid
    pub const fn offset_by(self, offset: isize) -> OffsetMetaHandle<T> {
        if offset.is_negative() {
            self.sub((-offset) as usize)
        } else {
            self.add(offset as usize)
        }
    }

    /// Cast this handle to any sized type, similar to [`NonNull::cast`][core::ptr::NonNull]
    #[inline]
    pub const fn cast<U>(self) -> OffsetMetaHandle<U> {
        OffsetMetaHandle::from_offset_meta(self.0, ())
    }

    /// Cast this handle to any unsized type with the same metadata as it currently holds
    #[inline]
    pub const fn cast_unsized<U>(self) -> OffsetMetaHandle<U>
    where
        T: Pointee<Metadata = <U as Pointee>::Metadata>,
        U: ?Sized,
    {
        OffsetMetaHandle::from_offset_meta(self.0, self.1)
    }

    /// Coerce this handle to a type which unsizes from the current type
    #[cfg(feature = "unsize")]
    pub const fn coerce<U: ?Sized>(self) -> OffsetMetaHandle<U>
    where
        T: Unsize<U>,
    {
        let ptr: *const T = ptr::from_raw_parts(ptr::null(), self.1);
        let meta = ptr::metadata(ptr as *const U);
        OffsetMetaHandle(self.0, meta)
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
            .field("offset", &self.0)
            .field("metadata", &self.1)
            .finish()
    }
}
