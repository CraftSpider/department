use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    InsufficientSpace(usize, Option<usize>),
    InvalidAlign(usize, usize),
    /// Only emit by multi-item storages, indicates the maximum number of items have been allocated
    /// at once. *Sometimes* freeing existing items can fix this.
    NoSlots,
    Unimplemented,
}

impl StorageError {
    pub fn exceeds_max() -> StorageError {
        StorageError::InsufficientSpace(0, Some(usize::MAX))
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::InsufficientSpace(expected, available) => {
                write!(f, "Insufficient space in storage. ")?;
                match available {
                    Some(usize::MAX) => write!(
                        f,
                        "Expected more than usize::MAX",
                    )?,
                    Some(available) => write!(
                        f,
                        "Expected {}, but only {} is available", expected, available
                    )?,
                    None => write!(
                        f,
                        "Expected {}, but less was available", expected
                    )?,
                }
            }
            StorageError::InvalidAlign(expected, actual) => write!(
                f,
                "Invalid align to store type. Expected layout of at least {}, but backing was {}",
                expected,
                actual
            )?,
            StorageError::NoSlots => write!(f, "Multi-element storage has run out of slots")?,
            StorageError::Unimplemented => write!(f, "Operation is not supported on this storage")?,
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StorageError {}
