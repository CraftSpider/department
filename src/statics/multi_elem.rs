use super::StorageCell;

/// Static multi-element storage implementation
/// TODO
pub struct MultiElement<S: 'static, const N: usize>(&'static StorageCell<[S; N]>);
