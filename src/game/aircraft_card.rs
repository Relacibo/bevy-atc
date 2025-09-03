use crate::game::aircraft::AircraftJustSpawned;
use crate::game::run_conditions::was_mouse_wheel_used;
use aviation_helper_rs::heading::Heading;
use bevy::ecs::component::Component;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use strum::{EnumIter, IntoEnumIterator};

use super::aircraft::Aircraft;
use super::control::{
    ControlMode, ControlState, control_mode_is_clearance_selection, control_mode_is_normal,
};
use super::{GameState, Z_AIRCRAFT_CARD};

const AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.1,
    green: 0.3,
    blue: 0.1,
    alpha: 0.5,
};
const NORMAL_AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.1,
    green: 0.1,
    blue: 0.1,
    alpha: 0.7,
};
const SELECTED_AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.5,
    green: 0.5,
    blue: 0.1,
    alpha: 0.7,
};

const STEP_HEADING: f64 = 5.;
const STEP_HEADING_ACCEL: f64 = 30.;
const STEP_SPEED: f64 = 10.;
const STEP_SPEED_ACCEL: f64 = 50.;
const STEP_ALTITUDE: f64 = 500.;
const STEP_ALTITUDE_ACCEL: f64 = 5000.;

#[derive(Clone, Debug)]
pub struct AircraftCardPlugin;

impl Plugin for AircraftCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_aircraft_card_display_materials)
            .add_systems(
                Update,
                (
                    handle_escape_clear_selected.run_if(input_just_pressed(KeyCode::Escape)),
                    handle_clear_selected_on_any_click,
                    update_aircraft_card,
                    handle_aircraft_just_spawned,
                    update_pinned,
                    (
                        handle_aircraft_card_display_press.run_if(control_mode_is_normal),
                        handle_card_scroll
                            .run_if(control_mode_is_clearance_selection.and(was_mouse_wheel_used)),
                    )
                        .after(handle_clear_selected_on_any_click),
                )
                    .run_if(in_state(GameState::Running)),
            );
    }
}

#[derive(Clone, Debug, Component)]
pub struct AircraftCard;

#[derive(Debug, Clone, Copy, Component, EnumIter, PartialEq)]
pub enum AircraftCardDisplay {
    Callsign,
    ClearedHeading,
    Heading,
    ClearedSpeed,
    Speed,
    ClearedAltitude,
    Altitude,
}

// Relationship-Komponente: Die AircraftCard ist an ein Aircraft "angeheftet" und speichert die relative Transformation
#[derive(Component)]
pub struct PinnedTo {
    pub entity: Entity,
    pub relative_translation: Vec3,
}

// Resource für die Material-Handles
#[derive(Resource, Clone)]
pub struct AircraftCardDisplayMaterials {
    pub normal: Handle<ColorMaterial>,
    pub selected: Handle<ColorMaterial>,
}

fn setup_aircraft_card_display_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let normal = materials.add(Color::Srgba(NORMAL_AIRCRAFT_CARD_COLOR));
    let selected = materials.add(Color::Srgba(SELECTED_AIRCRAFT_CARD_COLOR));
    commands.insert_resource(AircraftCardDisplayMaterials { normal, selected });
}

pub fn update_aircraft_card(
    q_aircraft_card: Query<(&Children, &PinnedTo), With<AircraftCard>>,
    q_aircraft: Query<&Aircraft>,
    q_card_children: Query<(&AircraftCardDisplay, &Children)>,
    mut q_text: Query<&mut Text2d>,
) {
    for (
        card_children,
        PinnedTo {
            entity: aircraft_entity,
            ..
        },
    ) in q_aircraft_card
    {
        let Ok(aircraft) = q_aircraft.get(*aircraft_entity) else {
            continue;
        };
        for card_child in card_children {
            let (display, text_children) = match q_card_children.get(*card_child) {
                Ok(val) => val,
                Err(_) => continue,
            };
            for &text_entity in text_children {
                if let Ok(mut text) = q_text.get_mut(text_entity) {
                    text.0 = match display {
                        AircraftCardDisplay::Callsign => aircraft.call_sign.clone(),
                        AircraftCardDisplay::ClearedHeading => aircraft
                            .cleared_heading
                            .as_ref()
                            .map(Heading::to_string)
                            .unwrap_or_default(),
                        AircraftCardDisplay::Heading => Heading::to_string(&aircraft.heading),
                        AircraftCardDisplay::ClearedSpeed => aircraft
                            .cleared_speed_knots
                            .map(display_speed)
                            .unwrap_or_default(),
                        AircraftCardDisplay::Speed => display_speed(aircraft.speed_knots),
                        AircraftCardDisplay::ClearedAltitude => aircraft
                            .cleared_altitude_feet
                            .map(display_altitude)
                            .unwrap_or_default(),
                        AircraftCardDisplay::Altitude => display_altitude(aircraft.altitude_feet),
                    };
                }
            }
        }
    }
}

fn display_speed(speed_knots: f64) -> String {
    speed_knots.floor().to_string()
}

fn display_altitude(altitude_feet: f64) -> String {
    (altitude_feet as i32 / 100).to_string()
}

pub fn handle_aircraft_just_spawned(
    mut events: EventReader<AircraftJustSpawned>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
    card_materials: Res<AircraftCardDisplayMaterials>,
) {
    use AircraftCardDisplay::*;
    for event in events.read() {
        let AircraftJustSpawned(aircraft_entity) = event;
        let mut children = Vec::new();
        let card_display = create_card_display_bundle(
            Callsign,
            meshes.add(Rectangle::new(71., 11.)),
            card_materials.normal.clone(),
            0.,
            18.,
            0.5,
        );
        let child_entity = commands.spawn(card_display).id();
        children.push(child_entity);
        let mesh = meshes.add(Rectangle::new(23.0, 11.0));
        let display_coords = [
            (ClearedHeading, -24., 6.),
            (Heading, 0., 6.),
            (ClearedSpeed, -24., -6.),
            (Speed, 0., -6.),
            (ClearedAltitude, -24., -18.),
            (Altitude, 0., -18.),
        ];
        for (display, x, y) in display_coords {
            let card_display = create_card_display_bundle(
                display,
                mesh.clone(),
                card_materials.normal.clone(),
                x,
                y,
                0.5,
            );
            let child_entity = commands.spawn(card_display).id();
            children.push(child_entity);
        }
        let relative_translation = Vec3::new(-80., 0., 0.);
        let mut entity = commands.spawn((
            AircraftCard,
            PinnedTo {
                entity: *aircraft_entity,
                relative_translation,
            },
            Mesh2d(meshes.add(Rectangle::new(74., 50.))),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_CARD_COLOR))),
            Transform::from_xyz(0., 0., Z_AIRCRAFT_CARD),
            Visibility::Visible,
        ));
        entity.add_children(&children);
    }
}

fn create_card_display_bundle(
    display: AircraftCardDisplay,
    mesh: Handle<Mesh>,
    card_material: Handle<ColorMaterial>,
    x: f32,
    y: f32,
    z: f32,
) -> impl Bundle {
    (
        display,
        Pickable::default(),
        Mesh2d(mesh),
        MeshMaterial2d(card_material),
        Transform::from_xyz(x, y, z),
        Visibility::Inherited,
        children![(
            Text2d::default(),
            TextLayout::new_with_justify(JustifyText::Justified),
            TextFont::from_font_size(100.),
            Transform::from_xyz(0., 0., 0.5).with_scale(Vec3 {
                x: 0.1,
                y: 0.1,
                z: 1.,
            }),
            Visibility::Inherited,
        )],
    )
}

// Entferne die Resource SelectedAircraftCardDisplay und die Komponente CameraScrollEnabled
// Passe die Auswahl- und Scroll-Logik auf ControlState/ControlMode an

// Auswahl-System: Setzt ControlMode::ClearanceSelection
#[allow(clippy::too_many_arguments)]
pub fn handle_aircraft_card_display_press(
    mut events: EventReader<Pointer<Pressed>>,
    q_card_display: Query<(Entity, &AircraftCardDisplay, &ChildOf)>,
    q_card: Query<&PinnedTo, With<AircraftCard>>, // für Aircraft-Entity
    mut q_aircraft: Query<&mut Aircraft>,         // für Clearance-Entfernung
    mut q_display: Query<(&AircraftCardDisplay, &mut MeshMaterial2d<ColorMaterial>)>,
    card_materials: Res<AircraftCardDisplayMaterials>,
    mut control_state: ResMut<ControlState>,
) {
    for event in events.read() {
        let Ok((display_entity, display, ChildOf(card_entity))) = q_card_display.get(event.target)
        else {
            continue;
        };
        let Ok(PinnedTo {
            entity: aircraft_entity,
            ..
        }) = q_card.get(*card_entity)
        else {
            continue;
        };
        // Rechtsklick: Clearance entfernen
        if event.button == PointerButton::Secondary {
            if let Ok(mut aircraft) = q_aircraft.get_mut(*aircraft_entity) {
                match display {
                    AircraftCardDisplay::ClearedHeading => aircraft.cleared_heading = None,
                    AircraftCardDisplay::ClearedSpeed => aircraft.cleared_speed_knots = None,
                    AircraftCardDisplay::ClearedAltitude => aircraft.cleared_altitude_feet = None,
                    _ => {}
                }
            }
            continue; // Keine Auswahl setzen
        }
        // Linksklick: Auswahl setzen
        control_state.mode = ControlMode::ClearanceSelection {
            aircraft_entity: *aircraft_entity,
            display_entity,
            display: *display,
        };
        if let Ok((_, mut display_material)) = q_display.get_mut(display_entity) {
            display_material.0 = card_materials.selected.clone();
        }
    }
}

// Auswahl löschen bei Klick auf Hintergrund oder Escape
pub fn handle_clear_selected_on_any_click(
    mut events: EventReader<Pointer<Pressed>>,
    mut q_display: Query<&mut MeshMaterial2d<ColorMaterial>>,
    card_materials: Res<AircraftCardDisplayMaterials>,
    mut control_state: ResMut<ControlState>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();
    if let ControlMode::ClearanceSelection { display_entity, .. } = &control_state.mode {
        if let Ok(mut display_material) = q_display.get_mut(*display_entity) {
            display_material.0 = card_materials.normal.clone();
        }
    }
    control_state.mode = ControlMode::Normal;
}

pub fn handle_escape_clear_selected(
    mut q_display: Query<&mut MeshMaterial2d<ColorMaterial>>,
    card_materials: Res<AircraftCardDisplayMaterials>,
    mut control_state: ResMut<ControlState>,
) {
    if let ControlMode::ClearanceSelection { display_entity, .. } = &control_state.mode {
        if let Ok(mut display_material) = q_display.get_mut(*display_entity) {
            display_material.0 = card_materials.normal.clone();
        }
    }
    control_state.mode = ControlMode::Normal;
}

// Scroll-System: Greift auf ControlMode::ClearanceSelection zu
pub fn handle_card_scroll(
    accumulated_mouse_scroll: Res<AccumulatedMouseScroll>,
    control_state: Res<ControlState>,
    mut q_aircraft: Query<&mut Aircraft>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let ControlMode::ClearanceSelection {
        aircraft_entity,
        display,
        ..
    } = &control_state.mode
    else {
        return;
    };
    let Ok(mut aircraft) = q_aircraft.get_mut(*aircraft_entity) else {
        return;
    };
    let delta: f64 = if accumulated_mouse_scroll.unit == MouseScrollUnit::Line {
        accumulated_mouse_scroll.delta.y as f64
    } else {
        (accumulated_mouse_scroll.delta.y / 100.).round() as f64
    };
    let ctrl = input.pressed(KeyCode::ControlLeft) || input.pressed(KeyCode::ControlRight);
    match display {
        AircraftCardDisplay::ClearedHeading => {
            let step = if ctrl {
                STEP_HEADING_ACCEL
            } else {
                STEP_HEADING
            };
            let new_val = calculate_cleared_value(
                aircraft.heading.get(),
                aircraft.cleared_heading.map(|h| h.get()),
                delta,
                step,
            );
            aircraft.cleared_heading = Some(Heading::from(new_val));
        }
        AircraftCardDisplay::ClearedSpeed => {
            let step = if ctrl { STEP_SPEED_ACCEL } else { STEP_SPEED };
            let new_val = calculate_cleared_value(
                aircraft.speed_knots,
                aircraft.cleared_speed_knots,
                delta,
                step,
            );
            aircraft.cleared_speed_knots = Some(new_val);
        }
        AircraftCardDisplay::ClearedAltitude => {
            let step = if ctrl {
                STEP_ALTITUDE_ACCEL
            } else {
                STEP_ALTITUDE
            };
            let new_val = calculate_cleared_value(
                aircraft.altitude_feet,
                aircraft.cleared_altitude_feet,
                delta,
                step,
            );
            aircraft.cleared_altitude_feet = Some(new_val);
        }
        _ => {}
    }
}

pub fn update_pinned(
    mut q_pinned: Query<(&PinnedTo, &mut Transform)>,
    q_target: Query<&Transform, Without<PinnedTo>>,
) {
    for (
        PinnedTo {
            entity: pinned_to_entity,
            relative_translation,
        },
        mut pinned_by_transform,
    ) in &mut q_pinned
    {
        let Ok(Transform {
            translation: target_translation,
            ..
        }) = q_target.get(*pinned_to_entity)
        else {
            continue;
        };
        pinned_by_transform.translation = target_translation + relative_translation;
    }
}

fn calculate_cleared_value(current: f64, cleared: Option<f64>, delta: f64, step: f64) -> f64 {
    let base = cleared.unwrap_or(current);
    let idx = base / step;
    let is_on_grid = (base % step).abs() == 0.;
    let go_downwards = delta < 0.0;

    let new_idx = match (is_on_grid, go_downwards) {
        (true, true) => idx - 1.0,
        (true, false) => idx + 1.0,
        (false, true) => idx.floor(),
        (false, false) => idx.ceil(),
    };
    new_idx * step
}
