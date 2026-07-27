use bevy::math::{IVec3, UVec3, Vec3};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CHUNK GEOMETRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const CHUNK_SIZE:   usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE; // 4096

/// Edge length of one block in world units. One, and every bit of the
/// pipeline assumes it — kept named so call sites read as intent rather
/// than as a magic `1.0`.
pub const BLOCK_SIZE: f32 = 1.0;

/// Split an in-space block position into `(chunk coord, local position)`.
/// Euclidean division, so negatives behave: block `(-1,0,0)` is chunk
/// `(-1,0,0)`, local `(15,0,0)`.
#[inline]
pub fn to_chunk_local(block: IVec3) -> (IVec3, UVec3) {
    let s = CHUNK_SIZE as i32;
    let chunk = block.div_euclid(IVec3::splat(s));
    (chunk, (block - chunk * s).as_uvec3())
}

/// Inverse of `to_chunk_local`.
#[inline]
pub fn to_space_pos(chunk: IVec3, local: UVec3) -> IVec3 {
    chunk * CHUNK_SIZE as i32 + local.as_ivec3()
}

/// Local translation of a chunk within its space.
#[inline]
pub fn chunk_translation(coord: IVec3) -> Vec3 {
    (coord * CHUNK_SIZE as i32).as_vec3()
}

/// Flat index of a local position inside a chunk's dense voxel array.
///
/// X changes fastest, which is cache-friendly for east-west sweeps:
/// `index = x | (y << 4) | (z << 8)`.
#[inline(always)]
pub fn local_index(x: usize, y: usize, z: usize) -> usize {
    debug_assert!(x < CHUNK_SIZE, "x={x} out of bounds");
    debug_assert!(y < CHUNK_SIZE, "y={y} out of bounds");
    debug_assert!(z < CHUNK_SIZE, "z={z} out of bounds");
    x | (y << 4) | (z << 8)
}

/// Inverse of `local_index`.
#[inline(always)]
pub fn local_from_index(i: usize) -> UVec3 {
    UVec3::new((i & 0xF) as u32, ((i >> 4) & 0xF) as u32, ((i >> 8) & 0xF) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_positions_land_in_the_right_chunk() {
        let (chunk, local) = to_chunk_local(IVec3::new(-1, 0, 0));
        assert_eq!(chunk, IVec3::new(-1, 0, 0));
        assert_eq!(local, UVec3::new(15, 0, 0));
    }

    #[test]
    fn chunk_local_roundtrips() {
        for p in [IVec3::ZERO, IVec3::new(17, -3, 48), IVec3::splat(-100)] {
            let (chunk, local) = to_chunk_local(p);
            assert_eq!(to_space_pos(chunk, local), p);
        }
    }

    #[test]
    fn local_index_roundtrips() {
        for i in 0..CHUNK_VOLUME {
            let p = local_from_index(i);
            assert_eq!(local_index(p.x as usize, p.y as usize, p.z as usize), i);
        }
    }
}
