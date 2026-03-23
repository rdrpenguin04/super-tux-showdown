use bevy::{
    dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin},
    prelude::*,
};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::Player;

#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub enum SelectedObject {
    Player(Entity),
}

pub fn plugin(app: &mut App) {
    app.add_plugins((DebugPickingPlugin, EguiPlugin::default()))
        .insert_resource(DebugPickingMode::Normal)
        .add_observer(selector)
        .add_systems(EguiPrimaryContextPass, debug_ui_system);
}

fn selector(mut evt: On<Pointer<Click>>, mut commands: Commands, players: Query<(), With<Player>>) {
    evt.propagate(false);
    if players.contains(evt.entity) {
        commands.insert_resource(SelectedObject::Player(evt.entity));
    } else {
        commands.remove_resource::<SelectedObject>();
    }
}

fn debug_ui_system(
    mut contexts: EguiContexts,
    mut angle: Local<i32>,
    obj: Option<Res<SelectedObject>>,
) -> Result {
    egui::Window::new("Debug").show(contexts.ctx_mut()?, |ui| {
        match obj.map(|x| x.into_inner()) {
            None => {
                ui.label("Selected object: none");
            }
            Some(SelectedObject::Player(e)) => {
                ui.label(format!("Selected object: Player ({e})"));
                ui.label("Launch angle:");
                ui.add(egui::Slider::new(&mut *angle, 0..=360));
                if ui.button("Send it").clicked() {}
            }
        }
    });
    Ok(())
}
