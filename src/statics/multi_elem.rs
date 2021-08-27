use super::StorageCell;

/// Static multi-element storage implementation
/// TODO
pub struct MultiElement<S: 'static>(&'static StorageCell<S>);
