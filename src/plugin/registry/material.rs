use crate::plugin::audio::block::SoundProfile;

pub enum ToolType {
    Pick,
    Hammer,
    Wrench,
    Shovel,
    Weapon,
}

pub enum WeaponType {
    Sword,
    Dagger,
    Spear,
    Longsword,
    Shield
}

pub struct MaterialDefinition {
    name: String,
    hardness: f32,
    required_tool: Some(ToolType),
    hardness_tier: u32,
    sound_profile: SoundProfile,
}