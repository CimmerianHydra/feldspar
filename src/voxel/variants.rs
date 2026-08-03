use crate::voxel::rotation::BlockRotation;
use crate::voxel::voxel::{Voxel, STATE_BITS, ROTATION_BITS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – MODEL IDENTITY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Index into the renderer's model arena. Cheap to copy, cheap to store 64
/// of.
///
/// The *identifier* lives down here in vocabulary while the arena it indexes
/// lives in `render`, for the same reason `BlockID` lives here and the
/// registry lives in `content`: a `BlockDefinition` has to name its geometry
/// without content depending on the renderer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ModelID(pub u32);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE VARIANT KEY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Which voxel bits select this block's model.
///
/// Deliberately *not* an arbitrary bitmask. Every real case selects whole
/// fields — a log wants rotation, a pipe wants six state bits, a rotatable
/// machine with a lit indicator wants both — so the descriptor names
/// fields and the index is a couple of shifts. No `PEXT`, no target
/// gating, no portability problem.
///
/// ## Sizing
///
/// The table is dense: `2^(bits)` entries of 4 bytes. A cube is 1 entry, a
/// slab 24, a pipe 64. Declare only the state bits that actually change
/// the *geometry* — flags that drive behaviour but not appearance
/// (powered, filtered, enabled) must stay out of the key or the table
/// doubles for nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VariantKey {
    /// Include the 5 rotation bits. 24 valid values, 32 table slots.
    pub uses_rotation: bool,
    /// How many of the low state bits participate.
    pub state_bits: u8,
}

/// Beyond this the dense table stops being obviously free and is almost
/// certainly a design error (all 11 state bits plus rotation would be
/// 64k entries = 256 KB for one block).
pub const MAX_VARIANT_BITS: u32 = 12;

impl VariantKey {
    pub const STATIC: Self = Self { uses_rotation: false, state_bits: 0 };

    pub const fn rotated() -> Self {
        Self { uses_rotation: true, state_bits: 0 }
    }

    pub const fn stateful(state_bits: u8) -> Self {
        Self { uses_rotation: false, state_bits }
    }

    pub const fn rotated_stateful(state_bits: u8) -> Self {
        Self { uses_rotation: true, state_bits }
    }

    #[inline]
    pub fn bits(self) -> u32 {
        (if self.uses_rotation { 5 } else { 0 }) + self.state_bits as u32
    }

    #[inline]
    pub fn table_len(self) -> usize {
        1usize << self.bits()
    }

    /// The dense index for a voxel. Rotation occupies the high part so
    /// that a rotated block's state variants stay contiguous, which keeps
    /// the common "same rotation, changing state" case (a pipe being
    /// reconnected) in one cache line.
    #[inline]
    pub fn index(self, voxel: Voxel) -> usize {
        let state_mask = (1u32 << self.state_bits) - 1;
        let state = (voxel.state() as u32) & state_mask;

        if self.uses_rotation {
            ((voxel.rotation().raw() as usize) << self.state_bits) | state as usize
        } else {
            state as usize
        }
    }

    /// Call once at registration. Catches the two mistakes that would
    /// otherwise surface as a panic in the mesher, thousands of frames later.
    pub fn validate(self, block_name: &str) {
        assert!(
            self.state_bits as u32 <= STATE_BITS,
            "block '{block_name}': variant key wants {} state bits, voxel has {STATE_BITS}",
            self.state_bits
        );
        assert!(
            self.bits() <= MAX_VARIANT_BITS,
            "block '{block_name}': variant key is {} bits ({} table entries) — \
             almost certainly including state that does not change geometry",
            self.bits(),
            self.table_len()
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE PER-BLOCK TABLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Lives on `BlockDefinition`. Maps a voxel to its geometry in one index.
///
/// Entries may repeat freely — a log declares all 5 rotation bits and gets
/// 32 entries pointing at the 3 models an axis-aligned log actually has.
/// Redundancy here costs 4 bytes an entry; the arena deduplicates the
/// expensive part.
#[derive(Clone, Debug)]
pub struct ModelTable {
    pub key:    VariantKey,
    pub models: Vec<ModelID>,
}

impl ModelTable {
    /// The single-model case: cubes, and anything else whose appearance
    /// never changes.
    pub fn single(id: ModelID) -> Self {
        Self { key: VariantKey::STATIC, models: vec![id] }
    }

    pub fn new(key: VariantKey, models: Vec<ModelID>) -> Self {
        debug_assert_eq!(
            models.len(),
            key.table_len(),
            "model table must be dense: {} entries for a {}-bit key",
            models.len(),
            key.bits()
        );
        Self { key, models }
    }

    /// A table keyed by rotation, from a possibly-sparse set of members.
    ///
    /// `set` and `models` are parallel — `models[i]` is the bake of
    /// `set[i]`. Every one of the 32 raw rotation slots gets a model:
    /// members get their own, and everything else — spins that this set
    /// doesn't distinguish, plus the 8 unreachable raw values 24..31 —
    /// falls back to `models[0]`.
    ///
    /// That fallback is why `RotationSet::rotations()` guarantees identity
    /// comes first: a voxel carrying a rotation the model wasn't baked for
    /// renders upright rather than indexing out of bounds.
    pub fn from_rotation_set(set: &[BlockRotation], models: Vec<ModelID>) -> Self {
        assert_eq!(
            set.len(),
            models.len(),
            "rotation set has {} members but {} models were baked",
            set.len(),
            models.len()
        );
        assert!(!models.is_empty(), "a rotation set must have at least one member");

        let mut table = vec![models[0]; 1 << ROTATION_BITS];
        for (rotation, id) in set.iter().zip(&models) {
            table[rotation.raw() as usize] = *id;
        }

        Self { key: VariantKey::rotated(), models: table }
    }

    /// Rotation × state. `models[i][s]` is `set[i]` at state value `s`.
    ///
    /// Index layout follows `VariantKey::index`: rotation in the high bits,
    /// so a block's state variants stay contiguous under a fixed rotation.
    pub fn from_rotation_state(
        set: &[BlockRotation],
        state_bits: u8,
        models: &[Vec<ModelID>],
    ) -> Self {
        assert_eq!(set.len(), models.len(), "rotation set and model rows disagree");
        assert!(!models.is_empty(), "a rotation set must have at least one member");

        let states = 1usize << state_bits;
        debug_assert!(models.iter().all(|row| row.len() == states));

        // Every rotation starts as a copy of the identity row, then members
        // overwrite theirs. Unreachable rotations therefore still vary
        // correctly with state, which matters for a connecting block: a
        // corrupted rotation should render an upright pipe with the right
        // arms, not an upright pipe with no arms.
        let mut table = Vec::with_capacity(states << ROTATION_BITS);
        for _ in 0..(1 << ROTATION_BITS) {
            table.extend_from_slice(&models[0]);
        }

        for (rotation, row) in set.iter().zip(models) {
            let base = (rotation.raw() as usize) << state_bits;
            table[base..base + states].copy_from_slice(row);
        }

        Self { key: VariantKey::rotated_stateful(state_bits), models: table }
    }

    /// The hot lookup. One index, no branch beyond the descriptor.
    #[inline]
    pub fn resolve(&self, voxel: Voxel) -> ModelID {
        // Safe by construction: `index` masks to `key.bits()`, and the
        // table is `2^bits` long. Kept as a checked index anyway — it is
        // one compare against a value already in cache.
        self.models[self.key.index(voxel)]
    }
}

impl Default for ModelTable {
    fn default() -> Self {
        Self { key: VariantKey::STATIC, models: Vec::new() }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::direction::Direction;
    use crate::voxel::rotation::BlockRotation;

    #[test]
    fn static_key_is_one_entry() {
        assert_eq!(VariantKey::STATIC.table_len(), 1);
        assert_eq!(VariantKey::STATIC.index(Voxel::full(7)), 0);
    }

    #[test]
    fn pipe_key_is_sixty_four() {
        let key = VariantKey::stateful(6);
        assert_eq!(key.table_len(), 64);

        let v = Voxel::full(3).with_state(0b101010);
        assert_eq!(key.index(v), 0b101010);
    }

    /// State bits above the declared width must not leak into the index —
    /// that is what lets a pipe carry non-geometric flags later.
    #[test]
    fn undeclared_state_bits_are_masked_out() {
        let key = VariantKey::stateful(6);
        let v = Voxel::full(3).with_state(0b111_1010_1010);
        assert_eq!(key.index(v), 0b101010);
    }

    #[test]
    fn rotation_and_state_compose() {
        let key = VariantKey { uses_rotation: true, state_bits: 2 };
        assert_eq!(key.table_len(), 128);

        let r = BlockRotation::from_parts(Direction::East, 1);
        let v = Voxel::full(1).with_rotation(r).with_state(0b11);
        assert_eq!(key.index(v), ((r.raw() as usize) << 2) | 0b11);
    }
}
