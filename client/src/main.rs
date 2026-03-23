pub mod data;
#[cfg(feature = "dev")]
pub mod debug;
pub mod input;

use avian2d::prelude::*;
use bevy::{asset::AssetMetaCheck, prelude::*};
#[cfg(feature = "dev")]
use bevy::{
    dev_tools::fps_overlay::FpsOverlayPlugin,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
};
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
            PhysicsPlugins::default(), // .set(PhysicsInterpolationPlugin::interpolate_all()),
            PhysicsPickingPlugin,
            PhysicsDebugPlugin,
        ))
        .add_plugins((data::plugin, input::plugin))
        .insert_gizmo_config(
            PhysicsGizmos {
                axis_lengths: Some(vec2(0.2, 0.2)),
                ..default()
            },
            GizmoConfig::default(),
        )
        .insert_resource(Gravity::default())
        .add_systems(Startup, load_temp_assets)
        .add_systems(OnEnter(MainState::Game), setup_game)
        .add_systems(Update, await_load.run_if(in_state(MainState::Loading)))
        .add_systems(
            FixedUpdate,
            (check_ground, player_movement, run_move_and_slide)
                .chain()
                .run_if(in_state(MainState::Game)),
        )
        .init_state::<MainState>()
        .run()
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Action)]
struct Player;

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
        .spawn((CameraRoot, Transform::from_xyz(0.0, 0.0, 10.0)))
        .with_children(|commands| {
            commands.spawn(Camera3d::default());
            commands.spawn(DirectionalLight {
                illuminance: 2000.0,
                ..default()
            });
            // commands.spawn((
            //     Camera2d::default(),
            //     Projection::Perspective(PerspectiveProjection::default()),
            //     Camera {
            //         order: 1,
            //         clear_color: ClearColorConfig::None,
            //         ..default()
            //     },
            // ));
        });
    commands.spawn((
        Collider::convex_polyline(vec![
            vec2(-4.0, 0.0),
            vec2(4.0, 0.0),
            vec2(3.2, -0.8),
            vec2(-3.2, -0.8),
        ])
        .unwrap(),
        RigidBody::Static,
    ));
    let idle = &character.anims[IDLE].frames[0];
    commands
        .spawn((
            Player,
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
                movement_velocity.x = held.direction * 0.5;
                damping = 5.0;
            }
            Action::Jumpsquat { frames_left, short } => {
                if !held.jump {
                    *short = true;
                }
                *frames_left -= 1;
                if *frames_left == 0 {
                    lin_vel.y = if *short { 4.8 } else { 8.0 };
                    *action = Action::Airborne {
                        coyote_frames: 0,
                        jumps_left: 1,
                        fast_fall: false,
                    };
                }
                damping = 20.0;
            }
        }

        if let Some(next_action) = buffer.try_consume(action.allowed()) {
            match (&mut *action, next_action) {
                (Action::Idle, InputAction::Jump) => {
                    *action = Action::Jumpsquat {
                        frames_left: 6,
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
                    }
                    lin_vel.y = 8.0;
                    *fast_fall = false;
                }
                _ => unreachable!(),
            }
        }

        // movement_velocity += Vec2::Y * 20.0

        movement_velocity = movement_velocity * delta_time * 40.0;

        lin_vel.0 += movement_velocity;

        lin_vel.0 += gravity.0 * delta_time * gravity_damping * 4.0;

        let current_speed = lin_vel.x.abs();
        if current_speed > 0.0 {
            lin_vel.0.x = lin_vel.0.x / current_speed
                * (current_speed - current_speed * damping * delta_time).max(0.0);
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
            |hit| {
                // println!("{hit:?}");
                MoveAndSlideHitResponse::Accept
            },
        );

        transform.translation = position.extend(0.0);
        lin_vel.0 = projected_velocity;
    }
}
