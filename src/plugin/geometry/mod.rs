pub mod collision;
pub mod element;
pub mod meshing;
pub mod model;
pub mod rotation;
pub mod shapes;
pub mod variants;
pub mod voxel;
pub mod quads;

pub mod prelude {
    pub use super::element::{BakedQuad, CullTag, Element, ElementBox, FaceSpec};
    pub use super::model::{BakedModel, ModelArena, ModelFlags, ModelId};
    pub use super::rotation::{dir_index, BlockRotation, ALL_DIRECTIONS, ROTATION_COUNT};
    pub use super::shapes::{ConnectionMask, pipe_slots, slots};
    pub use super::variants::{ModelTable, VariantKey};
}
