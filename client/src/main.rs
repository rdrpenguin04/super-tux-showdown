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
use super_tux_showdown_common::{TerrainBox, anim::names::IDLE};

use crate::{
    data::CharacterDescription,
    input::{ActionBuffer, HeldInputs, InputAction},
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
        .add_message::<Launch>()
        .add_systems(Startup, load_temp_assets)
        .add_systems(OnEnter(MainState::Game), setup_game)
        .add_systems(Update, await_load.run_if(in_state(MainState::Loading)))
        .add_systems(Update, update_game_ui.run_if(in_state(MainState::Game)))
        .add_systems(
            FixedUpdate,
            (
                apply_launches,
                check_ground,
                player_movement,
                run_move_and_slide,
            )
                .chain()
                .run_if(in_state(MainState::Game)),
        )
        .init_state::<MainState>()
        .run()
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Action)]
struct Player {
    damage: f32,
    weight: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            damage: 0.0,
            weight: 100.0,
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
        coyote_frames: u8,
        jumps_left: u8,
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
}

impl Action {
    pub fn allowed(&self) -> &'static [InputAction] {
        match self {
            Self::Idle => &[InputAction::Jump],
            Self::Airborne {
                coyote_frames,
                jumps_left,
                ..
            } => {
                if *jumps_left > 0 || *coyote_frames > 0 {
                    &[InputAction::Jump]
                } else {
                    &[]
                }
            }
            Self::Jumpsquat { .. } => &[],
            Self::Hitstun { .. } => &[],
            Self::Landing { .. } => &[],
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
            Transform::from_scale(Vec3::splat(1.0 / 256.0)).with_translation(vec3(0.0, -0.8, 0.0)),
            TextLayout::new_with_justify(Justify::Center),
            RenderLayers::layer(UI_RENDER_LAYER),
            Text2d::new("Tux:\n"),
        ))
        .with_children(|parent| {
            parent.spawn((TextSpan::new("0.0"), PlayerDamageLabel { player: 0 }));
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
    ));
    let idle = &character.anims[IDLE].frames[0];
    commands
        .spawn((
            Player::default(),
            to_collider(idle.bounding_box),
            RigidBody::Kinematic,
            CustomPositionIntegration,
            Transform::from_xyz(0.0, 0.2, 0.0),
            Visibility::Visible,
        ))
        .with_children(|parent| {
            parent.spawn((
                SceneRoot(character.model.clone()),
                Transform::from_rotation(character.right_rot),
            ));
            parent.spawn((
                Transform::from_translation(idle.bounding_box.bottom.extend(0.0)),
                GroundDetector,
            ));
        });
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
    mut damage_label: Single<&mut TextSpan, With<PlayerDamageLabel>>,
    player: Single<&Player>,
) {
    damage_label.0 = format!("{:.1}", player.damage);
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
        println!("{knockback:?} {direction:?} {velocity:?} {action:?}");
    }
}

fn player_movement(
    mut query: Query<(&mut LinearVelocity, Has<Grounded>, &mut Action), With<Player>>,
    time: Res<Time<Fixed>>,
    held: Res<HeldInputs>,
    mut buffer: ResMut<ActionBuffer>,
    gravity: Res<Gravity>,
) {
    let delta_time = time.delta_secs();
    for (mut lin_vel, grounded, mut action) in &mut query {
        let mut movement_velocity = Vec2::ZERO;
        let mut damping = 10.0;
        let mut gravity_damping = 0.5;

        match &mut *action {
            Action::Idle => {
                if !grounded {
                    *action = Action::Airborne {
                        coyote_frames: 4,
                        jumps_left: 1,
                        fast_fall: false,
                    };
                }
                movement_velocity.x = held.direction;
            }
            Action::Airborne {
                coyote_frames,
                fast_fall,
                ..
            } => {
                *coyote_frames = coyote_frames.saturating_sub(1);
                if held.fast_fall && lin_vel.y <= 0.05 {
                    *fast_fall = true;
                    lin_vel.y -= 1.0;
                }
                if *fast_fall {
                    gravity_damping = 1.0;
                }
                if grounded {
                    *action = Action::Idle;
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
                    *action = Action::Airborne {
                        coyote_frames: 0,
                        jumps_left: 1,
                        fast_fall: false,
                    };
                }
                // damping = 10.0;
            }
            Action::Hitstun { frames_left } => {
                *frames_left -= 1;
                let frames_left = *frames_left; // Copy the value so we don't reference a non-existent object
                if frames_left == 0 {
                    *action = Action::Airborne {
                        coyote_frames: 0,
                        jumps_left: 1,
                        fast_fall: false,
                    };
                }
                if grounded {
                    *action = Action::Landing {
                        frames_left: 10.min(frames_left),
                    };
                }
                damping = 0.1;
            }
            Action::Landing { frames_left } => {
                *frames_left -= 1;
                if *frames_left == 0 {
                    *action = Action::Idle;
                }
                if !grounded {
                    *action = Action::Airborne {
                        coyote_frames: 0,
                        jumps_left: 1,
                        fast_fall: false,
                    };
                }
            }
        }

        if let Some(next_action) = buffer.try_consume(action.allowed()) {
            match (&mut *action, next_action) {
                (Action::Idle, InputAction::Jump) => {
                    *action = Action::Jumpsquat {
                        frames_left: 5,
                        short: false,
                    }
                }
                (
                    Action::Airborne {
                        coyote_frames,
                        jumps_left,
                        fast_fall,
                    },
                    InputAction::Jump,
                ) => {
                    if *coyote_frames > 0 {
                        *coyote_frames = 0;
                    } else {
                        *jumps_left -= 1;
                        if held.direction.signum() != lin_vel.x.signum() && lin_vel.x > 0.1 {
                            lin_vel.x += held.direction * 4.0;
                        } else {
                            lin_vel.x += held.direction;
                        }
                    }
                    lin_vel.y = 13.0;
                    *fast_fall = false;
                }
                _ => unreachable!(),
            }
        }

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
    mut query: Query<(Entity, &mut Transform, &mut LinearVelocity, &Collider), With<Player>>,
    move_and_slide: MoveAndSlide,
    time: Res<Time<Fixed>>,
) {
    for (entity, mut transform, mut lin_vel, collider) in &mut query {
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
            &SpatialQueryFilter::from_excluded_entities([entity]),
            |_| MoveAndSlideHitResponse::Accept,
        );

        transform.translation = position.extend(0.0);
        lin_vel.0 = projected_velocity;
    }
}
