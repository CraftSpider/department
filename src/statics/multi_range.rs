use super::StorageCell;

/// Static multi-range storage implementation
/// TODO
pub struct MultiRange<S: 'static>(&'static StorageCell<S>);
