use bevy::prelude::*;

#[derive(Component)]
pub struct UiDropAreaFreeform;

#[derive(Component)]
pub struct UiDropAreaFixed;

/// This must be added to a Node if it's supposed to represent an item in the UI.
#[derive(Component, Clone)]
pub struct UiItem {
    index : u32,
    count : u32,
}


pub fn spawn_ui_crafting_area(){}
