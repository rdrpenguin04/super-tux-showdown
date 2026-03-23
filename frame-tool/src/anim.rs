use bevy::prelude::*;

#[derive(Component, Default, Deref, DerefMut, Reflect)]
#[reflect(Component)]
#[require(Transform, SmoothingSettings)]
pub struct TargetTransform(pub Transform);

#[derive(Component, Reflect)]
pub struct SmoothingSettings {
    pub translation_decay_rate: f32,
    pub rotation_decay_rate: f32,
    pub scale_decay_rate: f32,
}

impl Default for SmoothingSettings {
    fn default() -> Self {
        Self {
            translation_decay_rate: 20.0,
            rotation_decay_rate: 20.0,
            scale_decay_rate: 20.0,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, smooth_transform);
}

fn smooth_transform(
    mut transforms: Query<(&TargetTransform, &SmoothingSettings, &mut Transform)>,
    time: Res<Time>,
) {
    for (target, settings, mut transform) in &mut transforms {
        transform.translation.smooth_nudge(
            &target.translation,
            settings.translation_decay_rate,
            time.delta_secs(),
        );
        transform.rotation.smooth_nudge(
            &target.rotation,
            settings.rotation_decay_rate,
            time.delta_secs(),
        );
        transform
            .scale
            .smooth_nudge(&target.scale, settings.scale_decay_rate, time.delta_secs());
    }
}
