mod anim;

use core::f32;
use std::path::PathBuf;

#[cfg(feature = "dev")]
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
use bevy::{
    asset::{
        AssetPath,
        io::{
            AssetSourceBuilder,
            memory::{Dir, MemoryAssetReader},
        },
    },
    mesh::VertexAttributeValues,
    prelude::*,
    tasks::IoTaskPool,
};
use bevy_egui::{egui::RichText, *};
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use super_tux_showdown_common::{
    Character, TerrainBox,
    anim::{AnimEnd, CharacterAnimation, Frame, MirrorRule, names::IDLE},
};

use crate::anim::TargetTransform;

#[derive(Resource, Debug, Default)]
struct MemoryDir {
    dir: Dir,
}

#[derive(Resource, Reflect, Debug, Default)]
#[reflect(Resource)]
struct ModelFileName(String);

#[derive(States, Reflect, Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
#[reflect(State)] // why is this ReflectState and not ReflectStates?
enum EditState {
    #[default]
    NoMesh,
    LoadingMesh,
    PreliminarySetup,
    Main,
}

#[derive(SubStates, Reflect, Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
#[reflect(State)] // why is this ReflectState and not ReflectStates?
#[source(EditState = EditState::PreliminarySetup)]
enum PreliminaryTask {
    #[default]
    SetName,
    ForwardFacing,
    RightFacing,
    LeftFacing,
}

#[derive(Resource, Reflect, Clone, Eq, PartialEq, Hash, Debug)]
#[reflect(Resource)]
struct MeshLoadHandle(UntypedHandle);

#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
#[require(Transform, Visibility, TargetTransform)]
struct CharacterRoot;

#[derive(Resource, Reflect, Debug, Default)]
#[reflect(Resource)]
struct CharacterMeta {
    name: String,
    forward_rot: Quat,
    right_rot: Quat,
    left_rot: Quat,
}

#[derive(Component, Reflect, Clone, Copy, Default)]
#[reflect(Component)]
#[require(Transform, Visibility)]
struct CameraRoot;

fn main() -> AppExit {
    let memory_dir = MemoryDir::default();
    let memory_reader = MemoryAssetReader {
        root: memory_dir.dir.clone(),
    };
    App::new()
        .register_asset_source(
            "memory",
            AssetSourceBuilder::new(move || Box::new(memory_reader.clone())),
        )
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Window {
                    title: "SuperTuxShowdown Frame Data Creator".into(),
                    fit_canvas_to_parent: true,
                    ..default()
                }
                .into(),
                ..default()
            }),
            EguiPlugin::default(),
            anim::plugin,
            #[cfg(feature = "dev")]
            (RemotePlugin::default(), RemoteHttpPlugin::default()),
        ))
        .add_systems(Startup, startup_system)
        .add_systems(
            Update,
            (
                await_mesh_load.run_if(in_state(EditState::LoadingMesh)),
                spinny.run_if(in_state(PreliminaryTask::SetName)),
                face_forward.run_if(in_state(PreliminaryTask::ForwardFacing)),
                face_right
                    .run_if(in_state(PreliminaryTask::RightFacing).or(in_state(EditState::Main))),
                face_left.run_if(in_state(PreliminaryTask::LeftFacing)),
                draw_box.run_if(in_state(EditState::Main)),
            ),
        )
        .add_systems(EguiPrimaryContextPass, main_ui)
        .insert_resource(memory_dir)
        .init_resource::<CharacterMeta>()
        .init_state::<EditState>()
        .add_sub_state::<PreliminaryTask>()
        .run()
}

#[derive(Serialize, Deserialize)]
struct EditorData {
    version: String,
    state: String,
}

fn startup_system(mut commands: Commands) {
    commands.spawn((
        CameraRoot,
        Transform::from_xyz(0.0, 0.0, 3.0),
        Camera3d::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 2000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn(CharacterRoot);
}

// Yes, this is a complicated algorithm to be running every frame.
// TODO: do better.
/// `root_transform`: Used to "cancel out" the existing transform
fn find_extents(
    root_entity: Entity,
    root_transform: &GlobalTransform,
    new_transform: Transform,
    hierarchy: &Query<&Children>,
    mesh_query: &Query<(&Mesh3d, &GlobalTransform)>,
    mesh_assets: &Assets<Mesh>,
) -> Option<TerrainBox> {
    let mut max_extents = TerrainBox {
        top: vec2(0.0, -f32::INFINITY),
        bottom: vec2(0.0, f32::INFINITY),
        left: vec2(f32::INFINITY, 0.0),
        right: vec2(-f32::INFINITY, 0.0),
    };

    for child in hierarchy.iter_descendants(root_entity) {
        if let Ok((mesh, transform)) = mesh_query.get(child) {
            let relative_transform = transform.reparented_to(root_transform);
            let mesh = mesh_assets
                .get(mesh)
                .unwrap()
                .clone()
                .transformed_by(relative_transform.mul_transform(new_transform));
            if let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            {
                for &[x, y, _] in positions {
                    if x > max_extents.right.x {
                        max_extents.right = vec2(x, y);
                    }
                    if x < max_extents.left.x {
                        max_extents.left = vec2(x, y);
                    }
                    if y > max_extents.top.y {
                        max_extents.top = vec2(x, y);
                    }
                    if y < max_extents.bottom.y {
                        max_extents.bottom = vec2(x, y);
                    }
                }
            } else {
                panic!("mesh data in unexpected form?");
            }
        }
    }

    if max_extents.top.y.is_infinite() {
        // We didn't find a mesh; bail.
        return None;
    }

    // Now the max extents have been established; we're going to add a slight inward
    // tolerance to account for geometry not being perfectly flat and average the
    // two endpoints of "close to the edge" in each case
    let mut result_min = TerrainBox {
        top: vec2(f32::INFINITY, max_extents.top.y),
        bottom: vec2(f32::INFINITY, max_extents.bottom.y),
        left: vec2(max_extents.left.x, f32::INFINITY),
        right: vec2(max_extents.right.x, f32::INFINITY),
    };
    let mut result_max = TerrainBox {
        top: vec2(-f32::INFINITY, max_extents.top.y),
        bottom: vec2(-f32::INFINITY, max_extents.bottom.y),
        left: vec2(max_extents.left.x, -f32::INFINITY),
        right: vec2(max_extents.right.x, -f32::INFINITY),
    };

    for child in hierarchy.iter_descendants(root_entity) {
        if let Ok((mesh, transform)) = mesh_query.get(child) {
            let relative_transform = transform.reparented_to(root_transform);
            let mesh = mesh_assets
                .get(mesh)
                .unwrap()
                .clone()
                .transformed_by(relative_transform.mul_transform(new_transform));
            if let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            {
                for &[x, y, _] in positions {
                    if x / max_extents.right.x > 0.95 {
                        if y < result_min.right.y {
                            result_min.right.y = y;
                        }
                        if y > result_max.right.y {
                            result_max.right.y = y;
                        }
                    }
                    if x / max_extents.left.x > 0.95 {
                        if y < result_min.left.y {
                            result_min.left.y = y;
                        }
                        if y > result_max.left.y {
                            result_max.left.y = y;
                        }
                    }
                    if y / max_extents.top.y > 0.95 {
                        if x < result_min.top.x {
                            result_min.top.x = x;
                        }
                        if x > result_max.top.x {
                            result_max.top.x = x;
                        }
                    }
                    if y / max_extents.bottom.y > 0.95 {
                        if x < result_min.bottom.x {
                            result_min.bottom.x = x;
                        }
                        if x > result_max.bottom.x {
                            result_max.bottom.x = x;
                        }
                    }
                }
            } else {
                panic!("mesh data in unexpected form?");
            }
        }
    }

    let result = TerrainBox {
        top: (result_min.top + result_max.top) / 2.0,
        bottom: (result_min.bottom + result_max.bottom) / 2.0,
        left: (result_min.left + result_max.left) / 2.0,
        right: (result_min.right + result_max.right) / 2.0,
    };

    Some(result)
}

fn await_mesh_load(
    mut commands: Commands,
    mesh_load_handle: Res<MeshLoadHandle>,
    mesh_query: Query<(&Mesh3d, &GlobalTransform)>,
    model_root: Single<(Entity, &GlobalTransform), With<CharacterRoot>>,
    mesh_assets: Res<Assets<Mesh>>,
    hierarchy: Query<&Children>,
    mut next_edit_state: ResMut<NextState<EditState>>,
    asset_server: Res<AssetServer>,
) {
    if asset_server.is_loaded_with_dependencies(&mesh_load_handle.0) {
        let (root_entity, root_transform) = *model_root;
        if let Some(TerrainBox { top, bottom, .. }) = find_extents(
            root_entity,
            root_transform,
            Transform::IDENTITY,
            &hierarchy,
            &mesh_query,
            &mesh_assets,
        ) {
            let scene_root = hierarchy.get(model_root.0).unwrap()[0];
            commands
                .entity(scene_root)
                .insert(TargetTransform(Transform::from_xyz(
                    0.0,
                    -(top.y + bottom.y) / 2.0,
                    0.0,
                )));
            commands
                .entity(model_root.0)
                .insert(TargetTransform(Transform::from_rotation(Quat::look_to_lh(
                    Vec3::NEG_Z,
                    Vec3::Y,
                ))));
            commands.remove_resource::<MeshLoadHandle>();
            next_edit_state.set(EditState::PreliminarySetup);
        }
    }
}

fn spinny(mut target: Single<&mut TargetTransform, With<CharacterRoot>>, time: Res<Time>) {
    target.rotate_axis(Dir3::Y, time.delta_secs() * -0.2);
}

fn face_forward(
    mut target: Single<&mut TargetTransform, With<CharacterRoot>>,
    meta: Res<CharacterMeta>,
) {
    target.rotation = meta.forward_rot;
}

fn face_right(
    mut target: Single<&mut TargetTransform, With<CharacterRoot>>,
    meta: Res<CharacterMeta>,
) {
    target.rotation = meta.right_rot;
}

fn face_left(
    mut target: Single<&mut TargetTransform, With<CharacterRoot>>,
    meta: Res<CharacterMeta>,
) {
    target.rotation = meta.left_rot;
}

fn draw_box(
    mut gizmos: Gizmos,
    model_root: Single<(Entity, &GlobalTransform, &Transform), With<CharacterRoot>>,
    mesh_query: Query<(&Mesh3d, &GlobalTransform)>,
    mesh_assets: Res<Assets<Mesh>>,
    hierarchy: Query<&Children>,
) {
    let (root_entity, root_global_transform, root_transform) = *model_root;
    if let Some(bounding_box) = find_extents(
        root_entity,
        root_global_transform,
        *root_transform,
        &hierarchy,
        &mesh_query,
        &mesh_assets,
    ) {
        gizmos.linestrip_2d(
            [
                bounding_box.top,
                bounding_box.right,
                bounding_box.bottom,
                bounding_box.left,
            ],
            Color::linear_rgb(0.0, 0.0, 1.0),
        );
    }
}

#[derive(Debug)]
enum MenuMessage {
    MeshReady(PathBuf, Vec<u8>),
}

macro_rules! direction_button {
    ($ui:expr, $dir:expr, $text:expr, $field:expr) => {
        let direction = $dir;
        let mut text = RichText::new($text);
        if ($field.mul_vec3(Vec3::Z) - direction.mul_vec3(Vec3::Z)).length_squared() < 1e-6 {
            text = text.strong();
        }
        if $ui.button(text).clicked() {
            $field = direction;
        }
    };
}

fn main_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    // Jank to work around running out of parameters in Bevy's fake variadics
    (mut exit_writer, mut app_message_proxy): (
        MessageWriter<AppExit>,
        Local<Option<(Sender<MenuMessage>, Receiver<MenuMessage>)>>,
    ),
    memory_dir: ResMut<MemoryDir>,
    char_root: Single<Entity, With<CharacterRoot>>,
    asset_server: Res<AssetServer>,
    edit_state: Res<State<EditState>>,
    mut next_edit_state: ResMut<NextState<EditState>>,
    preliminary_task: Option<Res<State<PreliminaryTask>>>,
    mut next_preliminary_task: ResMut<NextState<PreliminaryTask>>,
    mut meta: ResMut<CharacterMeta>,
    model_file: Option<Res<ModelFileName>>,
    hierarchy: Query<&Children>,
    (global_transform_query, mesh_query): (
        Query<&GlobalTransform>,
        Query<(&Mesh3d, &GlobalTransform)>,
    ),
    mesh_assets: Res<Assets<Mesh>>,
) -> Result {
    let (sender, receiver) =
        app_message_proxy.get_or_insert_with(|| crossbeam_channel::unbounded());

    for msg in receiver.try_iter() {
        match msg {
            MenuMessage::MeshReady(name, data) => {
                memory_dir.dir.insert_asset(&name, data);
                commands.insert_resource(ModelFileName(name.to_string_lossy().into_owned()));

                let scene_handle = asset_server.load(
                    GltfAssetLabel::Scene(0)
                        .from_asset(AssetPath::from_path_buf(name).with_source("memory")),
                );

                commands
                    .entity(*char_root)
                    .despawn_children()
                    .with_child(SceneRoot(scene_handle.clone()));

                next_edit_state.set(EditState::LoadingMesh);
                commands.insert_resource(MeshLoadHandle(scene_handle.untyped()));
            }
        }
    }

    egui::TopBottomPanel::top("menubar").show(contexts.ctx_mut()?, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
                if ui.button("New project").clicked() {
                    let sender = sender.clone();
                    IoTaskPool::get()
                        .spawn(async move {
                            if let Some(handle) = rfd::AsyncFileDialog::new()
                                .set_title("Open a mesh file")
                                .add_filter("GLTF meshes", &["glb", "gltf"])
                                .pick_file()
                                .await
                            {
                                sender
                                    .send(MenuMessage::MeshReady(
                                        handle.file_name().into(),
                                        handle.read().await,
                                    ))
                                    .unwrap();
                            }
                        })
                        .detach();
                }
                if ui
                    .add_enabled(
                        matches!(**edit_state, EditState::PreliminarySetup | EditState::Main),
                        egui::Button::new("Save as..."),
                    )
                    .clicked()
                {
                    let editor_data = EditorData {
                        version: env!("CARGO_PKG_VERSION").into(),
                        state: match **edit_state {
                            EditState::NoMesh | EditState::LoadingMesh => {
                                unreachable!("button should be disabled")
                            }
                            EditState::PreliminarySetup => "preliminary".into(),
                            EditState::Main => "main".into(),
                        },
                    };
                    let true_root = hierarchy.get(*char_root).unwrap()[0];
                    let true_root_transform = global_transform_query.get(true_root).unwrap();
                    let bounding_box = find_extents(
                        true_root,
                        true_root_transform,
                        Transform::from_rotation(meta.right_rot),
                        &hierarchy,
                        &mesh_query,
                        &mesh_assets,
                    )
                    .unwrap();
                    let character = Character {
                        name: meta.name.clone(),
                        model_file: model_file
                            .expect("if this isn't set, the button should be disabled")
                            .0
                            .clone(),
                        forward_rot: meta.forward_rot,
                        right_rot: meta.right_rot,
                        left_rot: meta.left_rot,
                        anims: [(
                            IDLE.into(),
                            CharacterAnimation {
                                mirror_rule: MirrorRule::NoChange,
                                end_behavior: AnimEnd::Loop { frame: 0 },
                                frames: vec![Frame {
                                    important: false,
                                    bounding_box,
                                }],
                            },
                        )]
                        .into(),
                        editor_data: serde_json::to_vec(&editor_data).unwrap(),
                    };
                    let data = serde_json::to_vec(&character).unwrap();
                    IoTaskPool::get()
                        .spawn(async move {
                            if let Some(handle) = rfd::AsyncFileDialog::new()
                                .set_title("Save Project")
                                .add_filter("STS model", &["stsmodel"])
                                .save_file()
                                .await
                            {
                                handle.write(&data).await.unwrap();
                            }
                        })
                        .detach();
                }
                if ui.button("Quit").clicked() {
                    exit_writer.write(AppExit::Success);
                }
            })
        });
    });

    egui::TopBottomPanel::bottom("editor").show(contexts.ctx_mut()?, |ui| match **edit_state {
        EditState::NoMesh => {
            ui.label("No project loaded; open a new mesh from the File menu to get started!");
        }
        EditState::LoadingMesh => {
            ui.label("Loading...");
        }
        EditState::PreliminarySetup => match **preliminary_task.unwrap() {
            PreliminaryTask::SetName => {
                ui.horizontal(|ui| {
                    ui.label("Name your character:");
                    if ui.text_edit_singleline(&mut meta.name).lost_focus() {
                        meta.forward_rot = Quat::look_to_lh(Vec3::NEG_Z, Vec3::Y);
                        next_preliminary_task.set(PreliminaryTask::ForwardFacing);
                    }
                });
            }
            PreliminaryTask::ForwardFacing => {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "What angle should {} be at to face forward?",
                        meta.name
                    ));
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::X, Vec3::Y),
                        "+X",
                        meta.forward_rot
                    );
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::Z, Vec3::Y),
                        "+Z",
                        meta.forward_rot
                    );
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_X, Vec3::Y),
                        "-X",
                        meta.forward_rot
                    );
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_Z, Vec3::Y),
                        "-Z",
                        meta.forward_rot
                    );
                    if ui.button("Done").clicked() {
                        meta.right_rot = meta
                            .forward_rot
                            .mul_quat(Quat::from_axis_angle(Vec3::Y, f32::consts::PI / 2.0));
                        next_preliminary_task.set(PreliminaryTask::RightFacing);
                    }
                });
            }
            PreliminaryTask::RightFacing => {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "What angle should {} be at to face right?",
                        meta.name
                    ));
                    direction_button!(ui, Quat::look_to_lh(Vec3::X, Vec3::Y), "+X", meta.right_rot);
                    direction_button!(ui, Quat::look_to_lh(Vec3::Z, Vec3::Y), "+Z", meta.right_rot);
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_X, Vec3::Y),
                        "-X",
                        meta.right_rot
                    );
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_Z, Vec3::Y),
                        "-Z",
                        meta.right_rot
                    );
                    if ui.button("Done").clicked() {
                        meta.left_rot = meta
                            .right_rot
                            .mul_quat(Quat::from_axis_angle(Vec3::Y, f32::consts::PI));
                        next_preliminary_task.set(PreliminaryTask::LeftFacing);
                    }
                });
            }
            PreliminaryTask::LeftFacing => {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "What angle should {} be at to face left?",
                        meta.name
                    ));
                    direction_button!(ui, Quat::look_to_lh(Vec3::X, Vec3::Y), "+X", meta.left_rot);
                    direction_button!(ui, Quat::look_to_lh(Vec3::Z, Vec3::Y), "+Z", meta.left_rot);
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_X, Vec3::Y),
                        "-X",
                        meta.left_rot
                    );
                    direction_button!(
                        ui,
                        Quat::look_to_lh(Vec3::NEG_Z, Vec3::Y),
                        "-Z",
                        meta.left_rot
                    );
                    if ui.button("Done").clicked() {
                        next_edit_state.set(EditState::Main);
                    }
                });
            }
        },
        EditState::Main => {
            ui.label(format!("Hi, {}!", meta.name));
        }
    });

    Ok(())
}
