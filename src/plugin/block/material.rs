use crate::plugin::audio::block::SoundProfile;

#[derive(Default)]
pub enum ToolType {
    #[default]
    Pick,
    Hammer,
    Wrench,
    Shovel,
    Weapon,
}

#[derive(Default)]
pub enum WeaponType {
    #[default]
    Hand,
    Sword,
    Dagger,
    Spear,
    Longsword,
    Shield
}

#[derive(Default)]
pub struct BlockMaterial {
    name: String,
    hardness: f32,
    mass_of_block: f32,                     // The mass of a full block of this material
    required_tool: Option<ToolType>,        // Required tool type to initiate breaking the material
    hardness_tier: u32,                     // Tier-gating so that only certain materials can break certain others
}