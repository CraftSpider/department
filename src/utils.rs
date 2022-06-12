use core::alloc::Layout;
use core::ptr::Pointee;
use core::{mem, ptr};

use crate::error::Result;
use crate::error::StorageError;

pub(crate) fn layout_of<T: ?Sized + Pointee>(meta: T::Metadata) -> Layout {
    let pointer = ptr::from_raw_parts(ptr::null_mut(), meta);
    // SAFETY: The provided metadata is passed by value, and thus must be a valid instance of the
    //         metadata for `T`
    unsafe { Layout::for_value_raw::<T>(pointer) }
}

pub(crate) fn validate_layout<T: ?Sized + Pointee, S>(meta: T::Metadata) -> Result<()> {
    validate_layout_for::<S>(layout_of::<T>(meta))
}

pub(crate) fn validate_layout_for<S>(layout: Layout) -> Result<()> {
    let validated_size = layout.size() <= mem::size_of::<S>();
    let validated_layout = layout.align() <= mem::align_of::<S>();

    if validated_size && validated_layout {
        Ok(())
    } else if !validated_size {
        Err(StorageError::InsufficientSpace(
            layout.size(),
            Some(mem::size_of::<S>()),
        ))
    } else {
        Err(StorageError::InvalidAlign(
            layout.align(),
            mem::align_of::<S>(),
        ))
    }
}
