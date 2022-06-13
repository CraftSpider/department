use super::StorageCell;

mod sealed {
    use crate::statics;

    pub trait Sealed {}

    impl<S> Sealed for statics::SingleStatic<S> {}
    impl<S, const N: usize> Sealed for statics::MultiStatic<S, N> {}
}

/// Trait representing storages that can be created from a static `StorageCell`.
pub trait StaticStorage<S>: sealed::Sealed {
    /// Create an instance of the storage from an already locked `StorageCell`
    fn take_cell(cell: &'static StorageCell<S>) -> Self;
}
