//! VPX's math, as far as the physics uses it.

pub mod bbox;
pub mod def;
pub mod equations;
pub mod matrix;
pub mod matrix3d;
pub mod vector;

pub use bbox::{FRect, FRect3D};
pub use def::Mwc64x;
pub use matrix::Matrix3;
pub use matrix3d::Matrix3D;
pub use vector::{Vertex2D, Vertex3Ds, cross_product};
