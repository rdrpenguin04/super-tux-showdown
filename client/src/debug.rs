use avian2d::prelude::*;
use bevy::{
    dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin},
    prelude::*,
};
use bevy_egui::{EguiContext, EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::{Action, Launch, Player, angle_to_vector};

#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub enum SelectedObject {
    Player(Entity),
}

#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
struct LaunchParams {
    angle: u16,
    damage: f32,
    flip: bool,
    scale: f32,
    base: f32,
}

#[derive(GizmoConfigGroup, Reflect, Default)]
#[reflect(Default)]
struct DebugGizmos;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        DebugPickingPlugin, // TODO: make this disable-able
        EguiPlugin::default(),
    ))
    .insert_resource(DebugPickingMode::Normal)
    .insert_resource(LaunchParams {
        angle: 0,
        flip: false,
        damage: 0.0,
        scale: 0.0,
        base: 0.0,
    })
    .init_gizmo_group::<DebugGizmos>()
    .insert_gizmo_config(
        DebugGizmos,
        GizmoConfig {
            depth_bias: -1.0,
            ..default()
        },
    )
    .add_observer(selector)
    .add_systems(Update, render_debug_gizmos)
    .add_systems(EguiPrimaryContextPass, debug_ui_system);
}

fn selector(
    mut evt: On<Pointer<Click>>,
    mut commands: Commands,
    players: Query<(), With<Player>>,
    egui_contexts: Query<(), With<EguiContext>>,
) {
    if players.contains(evt.entity) {
        commands.insert_resource(SelectedObject::Player(evt.entity));
        evt.propagate(false);
    } else if egui_contexts.contains(evt.entity) {
        evt.propagate(false);
    } else {
        commands.remove_resource::<SelectedObject>();
    }
}

fn render_debug_gizmos(
    obj: Option<Res<SelectedObject>>,
    transforms: Query<&GlobalTransform>,
    centers_of_mass: Query<&ComputedCenterOfMass>,
    launch_angle: Res<LaunchParams>,
    mut gizmos: Gizmos<DebugGizmos>,
) {
    if let Some(obj) = obj.map(|x| x) {
        match *obj {
            SelectedObject::Player(e) => {
                let transform = transforms.get(e).unwrap();
                let center_of_mass = transform.translation()
                    + transform
                        .rotation()
                        .mul_vec3(centers_of_mass.get(e).unwrap().extend(0.0));
                gizmos.ray(
                    center_of_mass,
                    angle_to_vector(launch_angle.angle, launch_angle.flip).extend(0.0),
                    Color::srgb(0.0, 1.0, 1.0),
                );
            }
        }
    }
}

fn debug_ui_system(
    mut contexts: EguiContexts,
    mut launch_params: ResMut<LaunchParams>,
    mut launch_writer: MessageWriter<Launch>,
    players: Query<(&Player, &Action)>,
    obj: Option<Res<SelectedObject>>,
) -> Result {
    egui::Window::new("Debug").show(contexts.ctx_mut()?, |ui| {
        match obj.map(|x| x.into_inner()) {
            None => {
                ui.label("Selected object: none");
            }
            Some(SelectedObject::Player(e)) => {
                let (player, action) = players.get(*e).unwrap();
                ui.label(format!("Selected object: Player ({e})"));
                ui.label(format!("Data: {player:?}"));
                ui.label(format!("State: {action:?}"));
                ui.add(
                    egui::Slider::new(&mut launch_params.angle, 0..=355)
                        .text("Launch angle")
                        .step_by(5.0),
                );
                ui.checkbox(&mut launch_params.flip, "Flipped");
                ui.add(
                    egui::Slider::new(&mut launch_params.damage, 0.0..=40.0)
                        .text("Damage")
                        .step_by(0.5),
                );
                ui.add(
                    egui::Slider::new(&mut launch_params.scale, 0.0..=100.0)
                        .text("Knockback scaling")
                        .step_by(0.5),
                );
                ui.add(
                    egui::Slider::new(&mut launch_params.base, 0.0..=100.0)
                        .text("Base knockback")
                        .step_by(0.5),
                );
                if ui.button("Send it").clicked() {
                    launch_writer.write(Launch {
                        target: *e,
                        damage: launch_params.damage,
                        angle: launch_params.angle,
                        scaling: launch_params.scale,
                        base: launch_params.base,
                        flipped: launch_params.flip,
                    });
                }
            }
        }
    });
    Ok(())
}
