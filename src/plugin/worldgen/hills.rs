use bevy::prelude::*;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use crate::plugin::chunk::{VoxelChunk, CHUNK_SIZE};
use crate::plugin::voxel::Voxel;
use crate::plugin::worldgen::main::WorldGenerator;

use crate::plugin::loader::block_registry::BlockRegistry;

const SEA_LEVEL:      i32   = 0;
const AMPLITUDE:      i32   = 128;
const DIRT_THICKNESS: i32   = 3;
const OCTAVES:        usize = 4;
const FREQUENCY:      f64   = 1.0 / 256.0;
const PERSISTENCE:    f64   = 0.5;
const LACUNARITY:     f64   = 2.0;

#[derive(Clone, Debug)]
pub struct HillsGenerator {
    pub seed: u64,
    fbm: Fbm<Perlin>,

    // Pre-resolved blocks. Built once, copied freely.
    surface: Voxel,
    dirt:    Voxel,
    slate:   Voxel,
}

impl HillsGenerator {



    pub fn new(seed: u64, registry: &BlockRegistry) -> Self {

        fn resolve(name: String, registry: &BlockRegistry) -> Voxel {
            let id = registry.by_name(name.to_string()).unwrap_or_else(|| {
                panic!("HillsGenerator: required block '{}' not in registry", name)
            });
            Voxel::full(id.0)
        }
        
        let fbm = Fbm::<Perlin>::new(seed as u32)
            .set_octaves(OCTAVES)
            .set_frequency(FREQUENCY)
            .set_persistence(PERSISTENCE)
            .set_lacunarity(LACUNARITY);

        Self {
            seed,
            fbm,
            surface: resolve("grass".to_string(), registry),
            dirt:    resolve("dirt".to_string(), registry),
            slate:   resolve("slate".to_string(), registry),
        }
    }

    #[inline]
    fn surface_height(&self, world_x: i32, world_z: i32) -> i32 {
        let n = self.fbm
            .get([world_x as f64, world_z as f64])
            .clamp(-1.0, 1.0);
        SEA_LEVEL + (n * AMPLITUDE as f64).round() as i32
    }
}

impl WorldGenerator for HillsGenerator {
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        let chunk_base   = chunk_pos * CHUNK_SIZE as i32;
        let chunk_base_y = chunk_base.y;
        let chunk_top_y  = chunk_base_y + CHUNK_SIZE as i32 - 1;

        let max_surface  = SEA_LEVEL + AMPLITUDE;
        let min_surface  = SEA_LEVEL - AMPLITUDE;
        let min_dirt_low = min_surface - (DIRT_THICKNESS - 1);

        if chunk_base_y > max_surface {
            *out = VoxelChunk::empty();
            return;
        }
        if chunk_top_y < min_dirt_low {
            *out = VoxelChunk::filled(self.slate);
            return;
        }

        *out = VoxelChunk::empty();

        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let world_x = chunk_base.x + lx as i32;
                let world_z = chunk_base.z + lz as i32;

                let surface_y     = self.surface_height(world_x, world_z);
                let lowest_dirt_y = surface_y - (DIRT_THICKNESS - 1);

                for ly in 0..CHUNK_SIZE {
                    let world_y = chunk_base_y + ly as i32;

                    let voxel = if world_y > surface_y {
                        continue;
                    } else if world_y == surface_y {
                        self.surface
                    } else if world_y >= lowest_dirt_y {
                        self.dirt
                    } else {
                        self.slate
                    };

                    out.set(lx, ly, lz, voxel);
                }
            }
        }
    }
}