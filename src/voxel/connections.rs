use crate::voxel::direction::{dir_index, Direction, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONNECTION MASKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// This used to live in `shape.rs`, next to `BlockShape::Pipe`. It moved
// because the pipe stopped being a shape family: its geometry now comes out
// of a `.model.json` like any other imported model, and a connection mask
// is not a fact about *pipes* — it is a fact about any block whose model
// varies per face. Fences, walls, cables and conduits all want this and
// none of them want to be a `BlockShape` variant.
//
// ## One bit per face, not two
//
// An earlier design gave each face four states — disconnected, free,
// in-only, out-only — which is 4^6 = 4096 configurations and needs twelve
// bits. The voxel has eleven. Rather than reinterpret the rotation field to
// buy the twelfth, directionality became its own block: a one-way pipe,
// which is a wall in the connectivity graph plus a directed arc across it.
//
// So a face is connected or it is not, the mask is six bits, it fits the
// state field with five to spare, and nothing in `Voxel` had to move.

/// How many state bits a connection mask occupies.
pub const CONNECTION_BITS: u32 = 6;

/// Low mask over those bits.
pub const CONNECTION_STATE_MASK: u16 = (1 << CONNECTION_BITS) - 1;

/// Which of a block's six faces are joined to their neighbour, as stored in
/// the voxel's low six state bits.
///
/// Authored, never derived from neighbours — which is what keeps geometry a
/// pure function of the voxel's own 32 bits, with no cross-chunk dependency
/// and no cascade when a neighbour changes. Placement *seeds* the mask from
/// what is around it (see `sim::behaviors::blocks::pipe`), but that is a
/// one-time write, not a live dependency.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ConnectionMask(pub u8);

impl ConnectionMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(0b0011_1111);

    /// Read the mask out of a voxel's state field.
    #[inline]
    pub fn from_state(state: u16) -> Self {
        Self((state & CONNECTION_STATE_MASK) as u8)
    }

    /// Write it back, leaving the five bits above it alone.
    #[inline]
    pub fn into_state(self, state: u16) -> u16 {
        (state & !CONNECTION_STATE_MASK) | self.0 as u16
    }

    #[inline]
    pub fn has(self, d: Direction) -> bool {
        self.0 & (1 << dir_index(d)) != 0
    }

    #[inline]
    pub fn with(self, d: Direction, on: bool) -> Self {
        let bit = 1 << dir_index(d);
        Self(if on { self.0 | bit } else { self.0 & !bit })
    }

    #[inline]
    pub fn toggled(self, d: Direction) -> Self {
        Self(self.0 ^ (1 << dir_index(d)))
    }

    #[inline]
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// True when this cell is a junction rather than a straight run or an
    /// endpoint. The reduced routing graph keys off exactly this.
    #[inline]
    pub fn is_junction(self) -> bool {
        self.count() >= 3
    }

    pub fn directions(self) -> impl Iterator<Item = Direction> {
        ALL_DIRECTIONS
            .into_iter()
            .enumerate()
            .filter_map(move |(i, d)| (self.0 & (1 << i) != 0).then_some(d))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_mask_roundtrips() {
        let m = ConnectionMask::NONE
            .with(Direction::North, true)
            .with(Direction::Up, true);

        assert!(m.has(Direction::North) && m.has(Direction::Up));
        assert!(!m.has(Direction::South));
        assert_eq!(m.count(), 2);
        assert!(!m.is_junction());
        assert_eq!(m.toggled(Direction::North).count(), 1);
        assert_eq!(m.directions().count(), 2);
    }

    /// The five bits above the mask are somebody else's, and must survive a
    /// reconnection untouched. A pipe that lost a neighbouring block's
    /// state on every right-click would be a very quiet bug.
    #[test]
    fn high_state_bits_survive() {
        let state = 0b101_0000_0000u16 | 0b10_1010;
        let mask = ConnectionMask::from_state(state);

        assert_eq!(mask.0, 0b10_1010);
        assert_eq!(mask.with(Direction::North, true).into_state(state), state | 1);
        assert_eq!(ConnectionMask::NONE.into_state(state), 0b101_0000_0000);
    }

    #[test]
    fn all_is_every_face() {
        assert_eq!(ConnectionMask::ALL.count(), 6);
        assert!(ALL_DIRECTIONS.iter().all(|d| ConnectionMask::ALL.has(*d)));
    }
}