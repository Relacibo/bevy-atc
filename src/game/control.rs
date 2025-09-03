use super::aircraft_card::AircraftCardDisplay;
use bevy::prelude::*;

#[derive(Clone, Debug, Resource, PartialEq, Default)]
pub enum ControlMode {
    #[default]
    Normal,
    ClearanceSelection {
        aircraft_entity: Entity,
        display_entity: Entity,
        display: AircraftCardDisplay,
    },
}

#[derive(Clone, Debug, Resource, Default)]
pub struct ControlState {
    pub mode: ControlMode,
}

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControlState>();
    }
}

pub fn control_mode_is_normal(control_state: Option<Res<ControlState>>) -> bool {
    matches!(
        control_state.as_ref().map(|s| &s.mode),
        Some(ControlMode::Normal)
    )
}

pub fn control_mode_is_clearance_selection(control_state: Option<Res<ControlState>>) -> bool {
    matches!(
        control_state.as_ref().map(|s| &s.mode),
        Some(ControlMode::ClearanceSelection { .. })
    )
}
