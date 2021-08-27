use super::StorageCell;

mod sealed {
    pub trait Sealed {}

    impl<S> Sealed for crate::statics::SingleElement<S> {}
    impl<S> Sealed for crate::statics::MultiElement<S> {}
    impl<S> Sealed for crate::statics::SingleRange<S> {}
    impl<S> Sealed for crate::statics::MultiRange<S> {}
}

/// Trait representing storages that can be created from a static `StorageCell`.
pub trait StaticStorage<S>: sealed::Sealed {
    /// Create an instance of the storage from an already locked `StorageCell`
    fn take_cell(cell: &'static StorageCell<S>) -> Self;
}
