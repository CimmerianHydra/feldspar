use bevy::prelude::*;

pub struct CameraFollowPlugin;

impl Plugin for CameraFollowPlugin {
    fn build(&self, app: &mut App) {
        app

        .add_systems(PreUpdate, update_cameras_transform_to_targets);
    }
}

#[derive(Component)]
pub struct CameraFollow {
    pub target : Entity,
    pub offset : Vec3 // Set to Vec3::ZERO for true FPS
}

// UPDATE
pub fn update_cameras_transform_to_targets(
    targets: Query<&GlobalTransform>,
    mut cams: Query<(&mut Transform, &CameraFollow), With<Camera3d>>,
) {
    for (mut cam_t, follow) in &mut cams {
        if let Ok(target_gt) = targets.get(follow.target) {
            // Put camera at target + local offset, and match facing
            let basis = target_gt.compute_transform();
            cam_t.translation = basis.translation + basis.rotation * follow.offset;
            cam_t.rotation = basis.rotation;
        }
    }
}