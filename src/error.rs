//! The common error handling types used by `department`

use core::fmt;

/// A result with [`StorageError`] as its error type
pub type Result<T> = core::result::Result<T, StorageError>;

/// The error type returned by storages upon allocation failure
#[derive(Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// The storage didn't have enough space for the requested allocation
    InsufficientSpace {
        /// Space required for storage
        expected: usize,
        /// Space available to store into
        available: Option<usize>,
    },
    /// The storage alignment wasn't valid for the requested allocation
    InvalidAlign {
        /// Alignment required
        expected: usize,
        /// Alignment available to store into
        available: usize,
    },
    /// The maximum number of items have been stored at once. *Sometimes* freeing existing items
    /// can fix this.
    NoSlots,
    /// The requested operation isn't supported by this storage.
    Unimplemented,
}

impl StorageError {
    /// Create a `StorageError` which represents insufficient space where the requested space
    /// is greater than the maximum possible storage space ([`usize::MAX`])
    pub const fn exceeds_max() -> StorageError {
        StorageError::InsufficientSpace {
            expected: 0,
            available: Some(usize::MAX),
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::InsufficientSpace {
                expected,
                available,
            } => {
                write!(f, "Insufficient space in storage. ")?;
                match available {
                    Some(usize::MAX) if *expected == 0 => {
                        write!(f, "Expected more than usize::MAX")
                    }
                    Some(available) => write!(
                        f,
                        "Expected {}, but only {} is available",
                        expected, available
                    ),
                    None => write!(f, "Expected {}, but less was available", expected),
                }
            }
            StorageError::InvalidAlign {
                expected,
                available: actual,
            } => write!(
                f,
                "Invalid align to store type. Expected layout of at least {}, but backing was {}",
                expected, actual
            ),
            StorageError::NoSlots => write!(f, "Multi-element storage has run out of slots"),
            StorageError::Unimplemented => write!(f, "Operation is not supported on this storage"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StorageError {}
