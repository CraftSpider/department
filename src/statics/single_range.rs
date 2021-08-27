use super::StorageCell;

/// Static single-range storage implementation
/// TODO
pub struct SingleRange<S: 'static>(&'static StorageCell<S>);
