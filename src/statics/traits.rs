use super::StorageCell;

mod sealed {
    use crate::statics;

    pub trait Sealed {}

    // TODO: Fix with statics
    impl<S> Sealed for statics::SingleItem<S> {}
    impl<S, const N: usize> Sealed for statics::MultiItem<S, N> {}
    // impl<S, const N: usize> Sealed for statics::SingleRange<S, N> {}
    // impl<S, const N: usize, const M: usize> Sealed for statics::MultiRange<S, N, M> {}
}

/// Trait representing storages that can be created from a static `StorageCell`.
pub trait StaticStorage<S>: sealed::Sealed {
    /// Create an instance of the storage from an already locked `StorageCell`
    fn take_cell(cell: &'static StorageCell<S>) -> Self;
}
