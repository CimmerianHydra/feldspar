//! Block definitions, the registry that holds them, and the JSON that fills
//! it.
//!
//! `BlockID` itself lives one layer down in [`crate::voxel::ids`] — a voxel
//! stores one, and `space` names one in every `BlockEvent`, so the identifier
//! has to be reachable from below. It is re-exported here because this is
//! where it acquires meaning.


pub mod asset;
pub mod behaviors;
pub mod definition;
pub mod registry;
pub mod surface;
pub use crate::voxel::BlockID;

pub use asset::{
    populate_block_registry_sys, BlockDefinitionAsset,
    BlockDefinitionAssets, BlockMaterialAsset, FaceTexturesAsset, SoundProfileAsset,
};
pub use behaviors::{
    BlockBehavior, BlockBehaviorData, BlockBehaviorRegistry, BlockBehaviors,
    BlockFamily, BlockSpawnContext, RegisterBlockBehaviorExtension,
};
pub use definition::{
    BlockDefinition, BlockMaterial, FaceTextures, SoundProfile, TextureSlot,
    ToolType,
};
pub use registry::{BlockRegistry, ChunkPalette};
