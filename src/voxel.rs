//! # Layer 1 — vocabulary
//!
//! Pure data and math that every other layer speaks and none of them owns:
//! the packed voxel, the six directions, the rotation group, shape families,
//! model-variant keys, chunk coordinate math.
//!
//! There are no plugins here, no systems, and no `Resource`s. Nothing in
//! this module may import from any other module in the crate.
//!
//! Everything worth naming is re-exported at this level, so call sites read
//! `crate::voxel::Voxel` rather than `crate::voxel::voxel::Voxel`.

pub mod coords;
pub mod direction;
pub mod ids;
pub mod rotation;
pub mod shape;
pub mod variants;
#[allow(clippy::module_inception)]
pub mod voxel;

pub use ids::{BlockID, ItemID};

pub use coords::{
    chunk_translation, local_from_index, local_index, to_chunk_local, to_space_pos, BLOCK_SIZE,
    CHUNK_SIZE, CHUNK_VOLUME,
};
pub use direction::{dir_from_index, dir_from_ivec3, dir_index, Direction, ALL_DIRECTIONS};
pub use rotation::{BlockRotation, RotMatrix, ROTATION_COUNT};
pub use shape::{pipe_slots, slots, BlockShape, ConnectionMask, FACE_SLOTS};
pub use variants::{ModelID, ModelTable, VariantKey, MAX_VARIANT_BITS};
pub use voxel::{Voxel, ID_BITS, ROTATION_BITS, STATE_BITS};
