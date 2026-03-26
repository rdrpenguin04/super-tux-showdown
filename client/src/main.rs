pub mod data;
#[cfg(feature = "dev")]
pub mod debug;
pub mod input;

use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::{
    asset::AssetMetaCheck,
    camera::{
        ScalingMode,
        visibility::{Layer, RenderLayers},
    },
    prelude::*,
};
#[cfg(feature = "dev")]
use bevy::{
    dev_tools::fps_overlay::FpsOverlayPlugin,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
};
use bevy_hanabi::prelude::*;
use super_tux_showdown_common::{
    TerrainBox,
    anim::{self, names::IDLE},
};

use crate::{
    data::CharacterDescription,
    input::{ActionBuffer, HeldInputs, InputAction, InputConfig},
};

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Transform, Visibility)]
struct CameraRoot;

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum MainState {
    #[default]
    Loading,
    Game,
}

const MAIN_RENDER_LAYER: Layer = 0;
const UI_RENDER_LAYER: Layer = 1;

#[derive(PhysicsLayer, Default)]
pub enum GameLayer {
    #[default]
    Default,
    Ecb,
    TerrainDetector,
    Terrain,
}

#[derive(Resource, Reflect, Debug, Clone, Deref, DerefMut)]
#[reflect(Resource)]
struct Players(Vec<Entity>);

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Window {
                        title: "SuperTuxShowdown".into(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }
                    .into(),
                    ..default()
                }),
            #[cfg(feature = "dev")]
            (
                RemotePlugin::default(),
                RemoteHttpPlugin::default(),
                FpsOverlayPlugin::default(),
                debug::plugin,
            ),
        ))
        .add_plugins((
            PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()),
            PhysicsPickingPlugin,
            PhysicsDebugPlugin,
        ))
        .add_plugins(HanabiPlugin)
        .add_plugins((data::plugin, input::plugin))
        .insert_gizmo_config(
            PhysicsGizmos {
                axis_lengths: Some(vec2(0.2, 0.2)),
                ..default()
            },
            GizmoConfig {
                depth_bias: -1.0,
                ..default()
            },
        )
        .insert_resource(Gravity::default())
        .insert_resource(Players(vec![]))
        .add_message::<Launch>()
        .add_systems(Startup, load_temp_assets)
        .add_systems(OnEnter(MainState::Game), setup_game)
        .add_systems(Update, await_load.run_if(in_state(MainState::Loading)))
        .add_systems(Update, update_game_ui.run_if(in_state(MainState::Game)))
        .add_systems(
            FixedUpdate,
            (
                check_ground,
                apply_launches,
                player_movement,
                run_move_and_slide,
                hacky_update_animations,
                hacky_respawn,
            )
                .chain()
                .run_if(in_state(MainState::Game)),
        )
        .init_state::<MainState>()
        .run()
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Action, HeldInputs, ActionBuffer)]
pub struct Player {
    damage: f32,
    weight: f32,
    facing: f32,
    coyote_frames: u8,
    // specifically midair jumps
    jumps_left: u8,
    temp_data: anim::Frame,
    temp_data_2: (Quat, Quat),
}

impl Default for Player {
    fn default() -> Self {
        Self {
            damage: 0.0,
            weight: 100.0,
            facing: 1.0,
            coyote_frames: 0,
            jumps_left: 1,
            temp_data: anim::Frame::default(),
            temp_data_2: (Quat::IDENTITY, Quat::IDENTITY),
        }
    }
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
struct PlayerDamageLabel {
    player: u8,
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
struct Grounded;

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Sensor, Collider::circle(0.015))]
struct GroundDetector;

#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub enum Action {
    #[default]
    Idle,
    Airborne {
        fast_fall: bool,
    },
    Jumpsquat {
        frames_left: u8,
        short: bool,
    },
    Hitstun {
        frames_left: u8,
    },
    Landing {
        frames_left: u8,
    },
    Turnaround {
        frames_left: u8,
    },
}

impl Action {
    pub fn allowed(&self, player: &Player, lin_vel: &LinearVelocity) -> &'static [InputAction] {
        match self {
            Self::Idle => &[InputAction::Jump],
            Self::Airborne { .. } => {
                let jump = player.jumps_left > 0 || player.coyote_frames > 0;
                let fast_fall = lin_vel.y <= 0.1;
                if jump && fast_fall {
                    &[InputAction::Jump, InputAction::FastFall]
                } else if jump {
                    &[InputAction::Jump]
                } else if fast_fall {
                    &[InputAction::FastFall]
                } else {
                    &[]
                }
            }
            Self::Jumpsquat { .. } => &[],
            Self::Hitstun { .. } => &[],
            Self::Landing { .. } => &[],
            Self::Turnaround { .. } => &[InputAction::Jump],
        }
    }
}

#[derive(Resource)]
struct GameAssets {
    tux: Handle<CharacterDescription>,
}

fn load_temp_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GameAssets {
        tux: assets.load("models/tux.stsmodel"),
    });
}

fn await_load(
    assets: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    mut next_state: ResMut<NextState<MainState>>,
) {
    if assets.is_loaded_with_dependencies(&game_assets.tux) {
        next_state.set(MainState::Game);
    }
}

fn to_collider(x: TerrainBox) -> Collider {
    Collider::convex_polyline(vec![x.top, x.right, x.bottom, x.left]).unwrap()
}

fn setup_game(
    mut commands: Commands,
    characters: Res<Assets<CharacterDescription>>,
    game_assets: Res<GameAssets>,
) {
    let character = characters.get(&game_assets.tux).unwrap();
    commands
        .spawn((CameraRoot, Transform::from_xyz(0.0, 0.0, 24.0)))
        .with_children(|commands| {
            commands.spawn((Camera3d::default(), RenderLayers::layer(MAIN_RENDER_LAYER)));
            commands.spawn(DirectionalLight {
                illuminance: 2000.0,
                ..default()
            });
        });

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 2.0,
            },
            ..OrthographicProjection::default_2d()
        }),
        RenderLayers::layer(UI_RENDER_LAYER),
    ));

    commands
        .spawn((
            Transform::from_scale(Vec3::splat(1.0 / 256.0))
                .with_translation(vec3(-0.25, -0.8, 0.0)),
            TextLayout::new_with_justify(Justify::Center),
            RenderLayers::layer(UI_RENDER_LAYER),
            Text2d::new("Tux\n"),
        ))
        .with_children(|parent| {
            parent.spawn((TextSpan::new("0.0"), PlayerDamageLabel { player: 0 }));
            parent.spawn(TextSpan::new("%"));
        });

    commands
        .spawn((
            Transform::from_scale(Vec3::splat(1.0 / 256.0)).with_translation(vec3(0.25, -0.8, 0.0)),
            TextLayout::new_with_justify(Justify::Center),
            RenderLayers::layer(UI_RENDER_LAYER),
            Text2d::new("Tux\n"),
        ))
        .with_children(|parent| {
            parent.spawn((TextSpan::new("0.0"), PlayerDamageLabel { player: 1 }));
            parent.spawn(TextSpan::new("%"));
        });

    commands.spawn((
        Collider::convex_polyline(vec![
            vec2(-8.0, 0.0),
            vec2(8.0, 0.0),
            vec2(6.4, -1.6),
            vec2(-6.4, -1.6),
        ])
        .unwrap(),
        RigidBody::Static,
        CollisionLayers::new(
            GameLayer::Terrain,
            [GameLayer::Ecb, GameLayer::TerrainDetector],
        ),
    ));
    let idle = &character.anims[IDLE].frames[0];
    let p1 = commands
        .spawn((
            Player {
                temp_data: idle.clone(),
                temp_data_2: (character.left_rot, character.right_rot),
                ..default()
            },
            InputConfig::default(),
            to_collider(idle.bounding_box),
            RigidBody::Kinematic,
            CustomPositionIntegration,
            Transform::from_xyz(0.0, 0.2, 0.0),
            Visibility::Visible,
            CollisionLayers::new(GameLayer::Ecb, GameLayer::Terrain),
        ))
        .with_children(|parent| {
            parent.spawn((
                SceneRoot(character.model.clone()),
                Transform::from_rotation(character.right_rot),
            ));
            parent.spawn((
                Transform::from_translation(idle.bounding_box.bottom.extend(0.0)),
                GroundDetector,
                CollisionLayers::new(GameLayer::TerrainDetector, GameLayer::Terrain),
            ));
        })
        .id();
    let p2 = commands
        .spawn((
            Player {
                temp_data: idle.clone(),
                temp_data_2: (character.left_rot, character.right_rot),
                ..default()
            },
            to_collider(idle.bounding_box),
            RigidBody::Kinematic,
            CustomPositionIntegration,
            Transform::from_xyz(0.0, 0.2, 0.0),
            Visibility::Visible,
            CollisionLayers::new(GameLayer::Ecb, GameLayer::Terrain),
        ))
        .with_children(|parent| {
            parent.spawn((
                SceneRoot(character.model.clone()),
                Transform::from_rotation(character.right_rot),
            ));
            parent.spawn((
                Transform::from_translation(idle.bounding_box.bottom.extend(0.0)),
                GroundDetector,
                CollisionLayers::new(GameLayer::TerrainDetector, GameLayer::Terrain),
            ));
        })
        .id();
    commands.insert_resource(Players(vec![p1, p2]));
}

#[derive(Message)]
pub struct Launch {
    pub target: Entity,
    pub damage: f32,
    pub angle: u16,
    pub scaling: f32,
    pub base: f32,
    pub flipped: bool,
}

fn update_game_ui(
    mut damage_labels: Query<(&mut TextSpan, &PlayerDamageLabel)>,
    player_query: Query<&Player>,
    players_by_number: Res<Players>,
) {
    for (mut label, info) in &mut damage_labels {
        if let Some(player_id) = players_by_number.get(info.player as usize) {
            if let Ok(player) = player_query.get(*player_id) {
                label.0 = format!("{:.1}", player.damage);
            }
        }
    }
}

fn check_ground(
    mut commands: Commands,
    query: Query<(Entity, &ChildOf), With<GroundDetector>>,
    collisions: Collisions,
) {
    for (detector, parent) in &query {
        if collisions.collisions_with(detector).next().is_some() {
            commands.entity(parent.0).insert(Grounded);
        } else {
            commands.entity(parent.0).remove::<Grounded>();
        }
    }
}

pub fn angle_to_vector(mut angle: u16, flip: bool) -> Vec2 {
    if angle < 360 {
        if flip {
            angle = 180 - angle;
        }
        let angle = angle as f32 / 180.0 * PI;
        Vec2::from_angle(angle)
    } else {
        todo!("Sakurai + autolinks")
    }
}

fn apply_launches(
    mut commands: Commands,
    mut players: Query<(&mut Player, &mut Action, &mut LinearVelocity)>,
    mut launches: MessageReader<Launch>,
) {
    for launch in launches.read() {
        let (mut player, mut action, mut velocity) = players.get_mut(launch.target).unwrap();
        player.damage += launch.damage;
        let knockback = ((player.damage / 10.0 + player.damage * launch.damage / 20.0)
            * (200.0 / (player.weight + 100.0))
            * 1.4
            + 18.0)
            * (launch.scaling / 100.0)
            + launch.base;
        let direction = angle_to_vector(launch.angle, launch.flipped);
        velocity.0 = direction * knockback / 10.0;
        *action = Action::Hitstun {
            frames_left: ((knockback * 0.4) as u32).try_into().unwrap_or(255),
        };
        commands.entity(launch.target).remove::<Grounded>();
    }
}

fn player_movement(
    mut query: Query<(
        &mut Player,
        &mut LinearVelocity,
        Has<Grounded>,
        &mut Action,
        &HeldInputs,
        &mut ActionBuffer,
    )>,
    time: Res<Time<Fixed>>,
    gravity: Res<Gravity>,
) {
    let delta_time = time.delta_secs();
    for (mut player, mut lin_vel, grounded, mut action, held, mut buffer) in &mut query {
        let mut movement_velocity = Vec2::ZERO;
        let mut damping = 10.0;
        let mut gravity_damping = 0.5;

        // Jump handling
        player.coyote_frames = player.coyote_frames.saturating_sub(1);
        if grounded {
            player.coyote_frames = 0;
            player.jumps_left = 1;
        }

        // Tick active action
        match &mut *action {
            Action::Idle => {
                if !grounded {
                    *action = Action::Airborne { fast_fall: false };
                    player.coyote_frames = 4;
                }
                if held.direction != 0.0 {
                    if held.direction.signum() != player.facing {
                        *action = Action::Turnaround { frames_left: 11 };
                    } else {
                        movement_velocity.x = held.direction;
                    }
                }
            }
            Action::Airborne { fast_fall, .. } => {
                if *fast_fall {
                    lin_vel.y = -16.0;
                    gravity_damping = 1.0;
                }
                if grounded {
                    *action = Action::Landing { frames_left: 3 };
                }
                movement_velocity.x = held.direction * 0.2;
                damping = 0.1;
            }
            Action::Jumpsquat { frames_left, short } => {
                if !held.jump {
                    *short = true;
                }
                *frames_left -= 1;
                if *frames_left == 0 {
                    lin_vel.y = if *short { 10.0 } else { 13.0 };
                    *action = Action::Airborne { fast_fall: false };
                }
                // damping = 10.0;
            }
            Action::Hitstun { frames_left } => {
                *frames_left -= 1;
                let frames_left = *frames_left; // Copy the value so we don't reference a non-existent object
                if frames_left == 0 {
                    *action = Action::Airborne { fast_fall: false };
                }
                if grounded {
                    *action = Action::Landing { frames_left: 10 };
                }
                damping = 0.1;
            }
            Action::Landing { frames_left } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    *action = Action::Idle;
                }
                if !grounded {
                    *action = Action::Airborne { fast_fall: false };
                }
            }
            Action::Turnaround { frames_left } => {
                *frames_left -= 1;
                if !grounded {
                    *action = Action::Airborne { fast_fall: false };
                } else if *frames_left == 5 {
                    player.facing = -player.facing;
                } else if *frames_left == 0 {
                    *action = Action::Idle;
                }
            }
        }

        // Handle input
        if let Some(next_action) = buffer.try_consume(action.allowed(&player, &lin_vel)) {
            match (&mut *action, next_action) {
                (Action::Idle | Action::Turnaround { .. }, InputAction::Jump) => {
                    *action = Action::Jumpsquat {
                        frames_left: 5,
                        short: false,
                    }
                }
                (Action::Airborne { fast_fall }, InputAction::Jump) => {
                    if player.coyote_frames > 0 {
                        player.coyote_frames = 0;
                    } else {
                        player.jumps_left -= 1;
                        if held.direction.signum() != lin_vel.x.signum() && lin_vel.x > 0.1 {
                            lin_vel.x += held.direction * 4.0;
                        } else {
                            lin_vel.x += held.direction * 0.5;
                        }
                    }
                    lin_vel.y = 13.0;
                    *fast_fall = false;
                }
                (Action::Airborne { fast_fall }, InputAction::FastFall) => {
                    *fast_fall = true;
                }
                _ => unreachable!(),
            }
        }

        // Movement
        lin_vel.0 += movement_velocity;

        lin_vel.0 += gravity.0 * delta_time * gravity_damping * 4.0;

        let current_speed_x = lin_vel.x.abs();
        if current_speed_x > 0.0 {
            lin_vel.x = lin_vel.x / current_speed_x
                * (current_speed_x - current_speed_x * damping * delta_time).max(0.0);
        }
        let current_speed_y = lin_vel.y.abs();
        if current_speed_y > 0.0 {
            lin_vel.y = lin_vel.y / current_speed_y
                * (current_speed_y - current_speed_y * damping * delta_time).max(0.0);
        }
    }
}

fn run_move_and_slide(
    mut query: Query<(&mut Transform, &mut LinearVelocity, &Collider), With<Player>>,
    other_players_query: Query<Entity, With<Player>>,
    move_and_slide: MoveAndSlide,
    time: Res<Time<Fixed>>,
) {
    for (mut transform, mut lin_vel, collider) in &mut query {
        let MoveAndSlideOutput {
            position,
            projected_velocity,
        } = move_and_slide.move_and_slide(
            collider,
            transform.translation.xy(),
            0.0,
            lin_vel.0,
            time.delta(),
            &MoveAndSlideConfig::default(),
            &SpatialQueryFilter::from_excluded_entities(other_players_query),
            |_| MoveAndSlideHitResponse::Accept,
        );

        transform.translation = position.extend(0.0);
        lin_vel.0 = projected_velocity;
    }
}

fn hacky_update_animations(
    mut players: Query<(&Player, &mut Collider, &Children)>,
    mut scene_roots: Query<&mut Transform, With<SceneRoot>>,
) {
    for (player, mut collider, children) in &mut players {
        // TODO: anything but this bodge
        let mut bounding_box = player.temp_data.bounding_box;
        if player.facing == -1.0 {
            bounding_box = bounding_box.flip();
        }
        *collider = to_collider(bounding_box);
        for child in children {
            if let Ok(mut transform) = scene_roots.get_mut(*child) {
                transform.rotation = if player.facing == -1.0 {
                    player.temp_data_2.0
                } else {
                    player.temp_data_2.1
                };
            }
        }
    }
}

fn hacky_respawn(mut players: Query<(&mut Transform, &mut LinearVelocity, &mut Player)>) {
    for (mut transform, mut velocity, mut player) in &mut players {
        if transform.translation.y < -20.0 {
            transform.translation.x = 0.0;
            transform.translation.y = 5.0;
            velocity.x = 0.0;
            velocity.y = -5.0;
            player.damage = 0.0;
        }
    }
}
