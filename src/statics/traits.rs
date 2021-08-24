
use super::StorageCell;

mod sealed {
    pub trait Sealed {}

    impl<S> Sealed for crate::statics::SingleElement<S> {}
    impl<S> Sealed for crate::statics::MultiElement<S> {}
    impl<S> Sealed for crate::statics::SingleRange<S> {}
    impl<S> Sealed for crate::statics::MultiRange<S> {}
}

pub trait StaticStorage<S>: sealed::Sealed {
    fn take_cell(cell: &'static StorageCell<S>) -> Self;
}
