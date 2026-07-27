//! Registry identifiers.
//!
//! `BlockID` and `ItemID` sit in vocabulary rather than next to the registries
//! that hand them out, and that placement is load-bearing. A `BlockID` is what
//! the packed `Voxel` stores in its low 16 bits, what a `BlockEvent` carries,
//! and what an item points at when it places something; a `ItemID` is what a
//! recipe, a stack and an inventory total all key off. If either lived in
//! `content`, then `space` could not name the block a write just replaced, and
//! `block` and `item` could not reference each other without a cycle.
//!
//! Nothing here knows what a block or an item *is*. These are indices, and the
//! registries in [`crate::content`] are what give them meaning.

/// Index into `BlockRegistry`. Id 0 is always air.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockID(pub u16);

impl BlockID {
    pub const AIR: Self = Self(0);

    #[inline]
    pub fn is_air(self) -> bool { self.0 == 0 }
}

/// Index into `ItemRegistry`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemID(pub u16);
