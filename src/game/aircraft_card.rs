use crate::game::aircraft::AircraftJustSpawned;
use crate::game::run_conditions::was_mouse_wheel_used;
use aviation_helper_rs::types::heading::Heading;
use bevy::ecs::component::Component;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy::prelude::*;
use strum::EnumIter;

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

// Drag distance scaling constants
const DRAG_DISTANCE_BASE: f32 = 200.0; // Base distance in pixels
const DRAG_DISTANCE_SCALE_MIN: f32 = 1.0;
const DRAG_DISTANCE_SCALE_MAX: f32 = 3.0;

// Card scaling with camera zoom constants
const CARD_SCALE_MIN: f32 = 1.0;
const CARD_SCALE_MAX: f32 = 4.0;

// Global zoom constants
const ZOOM_SCALE_MIN: f32 = 0.5; // Camera zoom at which elements reach max scale
const ZOOM_SCALE_MAX: f32 = 4.0; // Camera zoom at which elements reach min scale

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
                    update_card_scale,
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

#[derive(Component)]
pub struct PinnedTo {
    pub entity: Entity,
    pub relative_translation: Vec3,
}

#[derive(Component)]
pub struct BeingDragged;

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

        entity
            .observe(on_card_drag_start)
            .observe(on_card_drag)
            .observe(on_card_drag_end);
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

#[allow(clippy::too_many_arguments)]
pub fn handle_aircraft_card_display_press(
    mut events: EventReader<Pointer<Pressed>>,
    q_card_display: Query<(Entity, &AircraftCardDisplay, &ChildOf)>,
    q_card: Query<&PinnedTo, With<AircraftCard>>,
    mut q_aircraft: Query<&mut Aircraft>,
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
    mut q_pinned: Query<(&mut PinnedTo, &mut Transform), Without<BeingDragged>>,
    q_target: Query<&Transform, Without<PinnedTo>>,
    camera_projection: Single<&Projection, With<Camera2d>>,
) {
    let scale = if let Projection::Orthographic(ortho) = &**camera_projection {
        ortho.scale
    } else {
        bevy::log::error!("Wrong camera projection. Expected orthographic!");
        return;
    };

    // Calculate current max drag distance based on camera zoom
    let normalized_zoom = (scale - ZOOM_SCALE_MIN) / (ZOOM_SCALE_MAX - ZOOM_SCALE_MIN);
    let clamped_zoom = normalized_zoom.clamp(0.0, 1.0);
    let distance_scale_factor = DRAG_DISTANCE_SCALE_MIN
        + (clamped_zoom * (DRAG_DISTANCE_SCALE_MAX - DRAG_DISTANCE_SCALE_MIN));
    let max_drag_distance = DRAG_DISTANCE_BASE * distance_scale_factor;

    for (mut pinned_to, mut pinned_by_transform) in &mut q_pinned {
        let Ok(Transform {
            translation: target_translation,
            ..
        }) = q_target.get(pinned_to.entity)
        else {
            continue;
        };

        // Check if current relative distance exceeds max drag distance
        let current_distance = (pinned_to.relative_translation.x
            * pinned_to.relative_translation.x
            + pinned_to.relative_translation.y * pinned_to.relative_translation.y)
            .sqrt();

        if current_distance > max_drag_distance {
            // Scale down the relative translation to fit within max drag distance
            let scale_factor = max_drag_distance / current_distance;
            pinned_to.relative_translation.x *= scale_factor;
            pinned_to.relative_translation.y *= scale_factor;
        }

        pinned_by_transform.translation = target_translation + pinned_to.relative_translation;
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
    // TODO: Delta (scroll delta) is not factored in yet
    new_idx * step
}

fn on_card_drag(
    trigger: Trigger<Pointer<Drag>>,
    mut cards: Query<(&mut PinnedTo, &mut Transform), With<AircraftCard>>,
    aircrafts: Query<&Transform, (With<Aircraft>, Without<AircraftCard>)>,
    camera_projection: Single<&Projection, With<Camera2d>>,
) {
    let Pointer {
        target,
        event: Drag { button, delta, .. },
        ..
    } = &trigger.event();

    let Ok((mut pinned_to, mut card_transform)) = cards.get_mut(*target) else {
        return;
    };

    if *button != PointerButton::Primary {
        return;
    }

    let scale = if let Projection::Orthographic(ortho) = &**camera_projection {
        ortho.scale
    } else {
        bevy::log::error!("Wrong camera projection. Expected orthographic!");
        return;
    };

    let Ok(aircraft_transform) = aircrafts.get(pinned_to.entity) else {
        return;
    };

    let current_relative = card_transform.translation - aircraft_transform.translation;

    let new_relative_x = current_relative.x + delta.x * scale;
    let new_relative_y = current_relative.y - delta.y * scale;

    let distance = (new_relative_x * new_relative_x + new_relative_y * new_relative_y).sqrt();

    // Calculate scaled max drag distance based on camera zoom
    let normalized_zoom = (scale - ZOOM_SCALE_MIN) / (ZOOM_SCALE_MAX - ZOOM_SCALE_MIN);
    let clamped_zoom = normalized_zoom.clamp(0.0, 1.0);
    let distance_scale_factor = DRAG_DISTANCE_SCALE_MIN
        + (clamped_zoom * (DRAG_DISTANCE_SCALE_MAX - DRAG_DISTANCE_SCALE_MIN));
    let max_drag_distance = DRAG_DISTANCE_BASE * distance_scale_factor;

    let final_relative = if distance <= max_drag_distance {
        Vec3::new(new_relative_x, new_relative_y, current_relative.z)
    } else {
        let scale_factor = max_drag_distance / distance;
        Vec3::new(
            new_relative_x * scale_factor,
            new_relative_y * scale_factor,
            current_relative.z,
        )
    };

    pinned_to.relative_translation = final_relative;
    card_transform.translation = aircraft_transform.translation + final_relative;
}

fn on_card_drag_start(trigger: Trigger<Pointer<DragStart>>, mut commands: Commands) {
    let target = trigger.event().target;
    commands.entity(target).insert(BeingDragged);
}

fn on_card_drag_end(trigger: Trigger<Pointer<DragEnd>>, mut commands: Commands) {
    let target = trigger.event().target;
    commands.entity(target).remove::<BeingDragged>();
}

/// Update aircraft card scale based on camera zoom level
/// Cards get larger when zooming out, smaller when zooming in, with min/max limits
pub fn update_card_scale(
    mut q_cards: Query<&mut Transform, With<AircraftCard>>,
    camera_projection: Single<&Projection, With<Camera2d>>,
) {
    let scale = if let Projection::Orthographic(ortho) = &**camera_projection {
        ortho.scale
    } else {
        bevy::log::error!("Wrong camera projection. Expected orthographic!");
        return;
    };

    // Calculate the scale factor for cards based on camera zoom
    // When camera scale is small (zoomed in), cards should be smaller
    // When camera scale is large (zoomed out), cards should be larger
    let normalized_zoom = (scale - ZOOM_SCALE_MIN) / (ZOOM_SCALE_MAX - ZOOM_SCALE_MIN);
    let clamped_zoom = normalized_zoom.clamp(0.0, 1.0);

    // Direct relationship: when zoomed out (higher scale), cards get larger
    let card_scale_factor = CARD_SCALE_MIN + (clamped_zoom * (CARD_SCALE_MAX - CARD_SCALE_MIN));

    for mut transform in &mut q_cards {
        transform.scale = Vec3::new(card_scale_factor, card_scale_factor, 1.0);
    }
}
