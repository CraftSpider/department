//! A simple type for defining a backing storage in a more declarative manner and more flexibly
//! than an int array

use core::{fmt, mem};

use crate::base::StorageSafe;

mod private {
    pub trait Align: Sized + Copy + Default {
        const DEFAULT: Self;
    }
}

use private::Align;

/// Give a [`Backing`] alignment 1
#[repr(align(1))]
#[derive(Copy, Clone, Default)]
pub struct Align1;
/// Give a [`Backing`] alignment 2
#[repr(align(2))]
#[derive(Copy, Clone, Default)]
pub struct Align2;
/// Give a [`Backing`] alignment 4
#[repr(align(4))]
#[derive(Copy, Clone, Default)]
pub struct Align4;
/// Give a [`Backing`] alignment 8
#[repr(align(8))]
#[derive(Copy, Clone, Default)]
pub struct Align8;
/// Give a [`Backing`] alignment 16
#[repr(align(16))]
#[derive(Copy, Clone, Default)]
pub struct Align16;

impl Align for Align1 {
    const DEFAULT: Self = Align1;
}
impl Align for Align2 {
    const DEFAULT: Self = Align2;
}
impl Align for Align4 {
    const DEFAULT: Self = Align4;
}
impl Align for Align8 {
    const DEFAULT: Self = Align8;
}
impl Align for Align16 {
    const DEFAULT: Self = Align16;
}

/// Standard type for a storage backing. The backing provided will have a size in
/// bytes of `N`, and an alignment of `A`.
#[derive(Copy, Clone)]
pub struct Backing<const N: usize, A: Align = Align1>([u8; N], A);

impl<const N: usize, A: Align> Backing<N, A> {
    /// Initialize a new backing
    pub const fn new() -> Backing<N, A> {
        Backing([0; N], A::DEFAULT)
    }
}

impl<const N: usize, A: Align> Default for Backing<N, A> {
    fn default() -> Self {
        Backing::new()
    }
}

impl<const N: usize, A: Align> fmt::Debug for Backing<N, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backing")
            .field("size", &N)
            .field("align", &mem::align_of::<A>())
            .finish()
    }
}

unsafe impl<const N: usize, A: Align> StorageSafe for Backing<N, A> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backing() {
        type Backing1 = Backing<1, Align1>;
        type Backing2 = Backing<2, Align2>;
        type Backing4 = Backing<4, Align4>;
        type Backing8 = Backing<8, Align8>;
        type Backing16 = Backing<16, Align16>;

        assert_eq!(mem::size_of::<Backing1>(), 1);
        assert_eq!(mem::align_of::<Backing1>(), 1);

        assert_eq!(mem::size_of::<Backing2>(), 2);
        assert_eq!(mem::align_of::<Backing2>(), 2);

        assert_eq!(mem::size_of::<Backing4>(), 4);
        assert_eq!(mem::align_of::<Backing4>(), 4);

        assert_eq!(mem::size_of::<Backing8>(), 8);
        assert_eq!(mem::align_of::<Backing8>(), 8);

        assert_eq!(mem::size_of::<Backing16>(), 16);
        assert_eq!(mem::align_of::<Backing16>(), 16);
    }
}
