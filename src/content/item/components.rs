use crate::voxel::BlockID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE ITEM COMPONENT BAG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One field per capability. Adding a capability = one field + one impl.
#[derive(Default, Clone, Debug)]
pub struct ItemComponents {
    places_block:   Option<PlacesBlock>,
    durability:     Option<Durability>,
    fuel:           Option<Fuel>,
}

/// Lets `ItemComponents` offer a uniform typed accessor.
pub trait ItemComponent: Sized {
    fn get(bag: &ItemComponents) -> Option<&Self>;
    fn set(bag: &mut ItemComponents, value: Self);
}

impl ItemComponents {
    pub fn get<T: ItemComponent>(&self) -> Option<&T> { T::get(self) }
    pub fn has<T: ItemComponent>(&self) -> bool { self.get::<T>().is_some() }
    pub fn with<T: ItemComponent>(mut self, value: T) -> Self {
        T::set(&mut self, value);
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE CAPABILITIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This item, used against a face, writes a block into the world.
///
/// The reason `BlockID` is vocabulary: an item names a block here, and a
/// block's drop will one day name an item, so neither module can own both
/// identifiers.
#[derive(Clone, Copy, Debug)]
pub struct PlacesBlock { pub block_id: BlockID }

#[derive(Clone, Copy, Debug)]
pub struct Durability { pub max: u32, pub current: u32 }

#[derive(Clone, Copy, Debug)]
pub struct Fuel { pub value: u32 }

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MACRO-BASED IMPL FOR EACH COMPONENT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stamps out the two-line impl. Adding a capability is one field in the
/// struct and one line here.
macro_rules! item_component {
    ($ty:ty, $field:ident) => {
        impl ItemComponent for $ty {
            fn get(bag: &ItemComponents) -> Option<&Self> { bag.$field.as_ref() }
            fn set(bag: &mut ItemComponents, v: Self) { bag.$field = Some(v); }
        }
    };
}

item_component!(PlacesBlock, places_block);
item_component!(Durability, durability);
item_component!(Fuel, fuel);
