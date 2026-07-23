use crate::plugin::loader::block_registry::BlockID;


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ItemComponents Struct
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
// Basic Definitions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


#[derive(Clone, Copy, Debug)]
pub struct PlacesBlock { pub block_id: BlockID }

#[derive(Clone, Copy, Debug)]
pub struct Durability { pub max: u32 }

#[derive(Clone, Copy, Debug)]
pub struct Fuel { pub value: u32 }


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Macro-based Implementation for each Component
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stamps out the two-line impl. Adding a capability is one field in the struct and
/// one line here.
macro_rules! item_component {
    ($ty:ty, $field:ident) => {
        impl ItemComponent for $ty {
            fn get(bag: &ItemComponents) -> Option<&Self> { bag.$field.as_ref() }
            fn set(bag: &mut ItemComponents, v: Self) { bag.$field = Some(v); }
        }
    };
}
// I fucking love Rust

item_component!(PlacesBlock, places_block);
// Expands into:
//
// impl ItemComponent for PlacesBlock {
//     fn get(bag: &ItemComponents) -> Option<&Self> { bag.places_block.as_ref() }
//     fn set(bag: &mut ItemComponents, v: Self) { bag.places_block = Some(v); }
// }

item_component!(Durability, durability);
item_component!(Fuel, fuel);