use bevy::ecs::component::Component;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use strum::{EnumIter, IntoEnumIterator};

use super::GameState;
use super::camera::CameraScrollEnabled;
use super::{AircraftJustSpawned, aircraft::Aircraft, heading::Heading};

const AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.3,
    blue: 0.1,
    alpha: 0.3,
};
const SELECTED_AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.8,
    green: 0.8,
    blue: 0.1,
    alpha: 0.7,
};

#[derive(Clone, Debug)]
pub struct AircraftCardPlugin;

impl Plugin for AircraftCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_aircraft_card_display_materials);
        app.add_systems(
            Update,
            (
                handle_escape_clear_selected.run_if(input_just_pressed(KeyCode::Escape)),
                handle_aircraft_card_display_press,
                handle_card_scroll,
                update_aircraft_card,
                handle_aircraft_just_spawned,
                update_pinned,
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
    let normal = materials.add(Color::srgba(0.0, 0.3, 0.1, 0.3));
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
                            .map(display_heading)
                            .unwrap_or_default(),
                        AircraftCardDisplay::Heading => display_heading(aircraft.heading),
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

fn display_heading(heading: Heading) -> String {
    heading.get().floor().to_string()
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
    for event in events.read() {
        let AircraftJustSpawned(aircraft_entity) = event;
        let mut children = Vec::new();
        for (index, display) in AircraftCardDisplay::iter().enumerate() {
            let mesh = meshes.add(Rectangle::new(60.0, 12.0));
            let text_entity = commands
                .spawn((
                    Text2d::default(),
                    TextLayout::new_with_justify(JustifyText::Justified),
                    TextFont::from_font_size(100.),
                    Transform::from_xyz(0., 0., 0.).with_scale(Vec3 {
                        x: 0.1,
                        y: 0.1,
                        z: 1.,
                    }),
                    Visibility::Inherited,
                ))
                .id();
            let child_entity = commands
                .spawn((
                    display,
                    Pickable::default(),
                    Mesh2d(mesh),
                    MeshMaterial2d(card_materials.normal.clone()),
                    Transform::from_xyz(0., 40. - index as f32 * 12., 1.),
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(child_entity).add_child(text_entity);
            children.push(child_entity);
        }
        let relative_translation = Vec3::new(-100., 0., 0.);
        let mut entity = commands.spawn((
            AircraftCard,
            PinnedTo {
                entity: *aircraft_entity,
                relative_translation,
            },
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(60., 60.),
            })),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_CARD_COLOR))),
            Transform::from_xyz(0., 0., 0.0), // z = 0.0 für Hintergrund
            Visibility::Visible,
        ));
        entity.add_children(&children);
    }
}

#[derive(Resource)]
pub struct SelectedAircraftCardDisplay {
    pub aircraft_entity: Entity,
    pub display_entity: Entity,
    pub display: AircraftCardDisplay,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_aircraft_card_display_press(
    mut events: EventReader<Pointer<Pressed>>,
    q_card_display: Query<(Entity, &AircraftCardDisplay, &ChildOf)>,
    q_card: Query<&PinnedTo, With<AircraftCard>>,
    mut q_display: Query<(&AircraftCardDisplay, &mut MeshMaterial2d<ColorMaterial>)>,
    card_materials: Res<AircraftCardDisplayMaterials>,
    selected_aircraft_card_display: Option<Res<SelectedAircraftCardDisplay>>,
    mut commands: Commands,
    mut q_camera: Single<&mut CameraScrollEnabled, With<Camera2d>>,
) {
    for event in events.read() {
        let Ok((display_entity, display, ChildOf(card_entity))) = q_card_display.get(event.target)
        else {
            continue;
        };
        println!("Es wurde ein display gedrückt! {display:?}");
        let Ok(PinnedTo {
            entity: aircraft_entity,
            ..
        }) = q_card.get(*card_entity)
        else {
            continue;
        };
        let old_display = selected_aircraft_card_display
            .as_ref()
            .map(|s| s.display_entity);
        // Kamera-Scroll deaktivieren
        q_camera.0 = false;
        commands.insert_resource(SelectedAircraftCardDisplay {
            aircraft_entity: *aircraft_entity,
            display_entity,
            display: *display,
        });

        if let Some(old_display) = old_display {
            if let Ok((_, mut display_material)) = q_display.get_mut(old_display) {
                display_material.0 = card_materials.normal.clone();
            }
        }
        if let Ok((_, mut display_material)) = q_display.get_mut(display_entity) {
            display_material.0 = card_materials.selected.clone();
        }
    }
}

pub fn handle_card_scroll(
    accumulated_mouse_scroll: Res<AccumulatedMouseScroll>,
    selected: Option<Res<SelectedAircraftCardDisplay>>,
    mut q_aircraft: Query<&mut Aircraft>,
) {
    if accumulated_mouse_scroll.delta.y == 0. {
        return;
    }
    let Some(selected) = selected else {
        return;
    };
    let Ok(mut aircraft) = q_aircraft.get_mut(selected.aircraft_entity) else {
        return;
    };

    let delta: f64 = if accumulated_mouse_scroll.unit == MouseScrollUnit::Line {
        accumulated_mouse_scroll.delta.y as f64
    } else {
        (accumulated_mouse_scroll.delta.y / 100.).round() as f64
    };
    match selected.display {
        AircraftCardDisplay::ClearedHeading => {
            const STEP: f64 = 5.;
            if let Some(ref mut heading) = aircraft.cleared_heading {
                *heading = heading.change(delta * STEP);
            } else {
                let base = if delta < 0.0 {
                    (aircraft.heading.get() / STEP).floor() * STEP
                } else {
                    (aircraft.heading.get() / STEP).ceil() * STEP
                };
                aircraft.cleared_heading = Some(Heading::from(base));
            }
        }
        AircraftCardDisplay::ClearedSpeed => {
            const STEP: f64 = 10.;
            if let Some(ref mut speed) = aircraft.cleared_speed_knots {
                *speed += delta * STEP;
            } else {
                let base = if delta < 0.0 {
                    (aircraft.speed_knots / STEP).floor() * STEP
                } else {
                    (aircraft.speed_knots / STEP).ceil() * STEP
                };
                aircraft.cleared_speed_knots = Some(base);
            }
        }
        AircraftCardDisplay::ClearedAltitude => {
            const STEP: f64 = 500.0;
            if let Some(ref mut alt) = aircraft.cleared_altitude_feet {
                *alt += delta * STEP;
            } else {
                let base = if delta < 0.0 {
                    (aircraft.altitude_feet / STEP).floor() * STEP
                } else {
                    (aircraft.altitude_feet / STEP).ceil() * STEP
                };
                aircraft.cleared_altitude_feet = Some(base);
            }
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
pub fn handle_escape_clear_selected(
    mut q_display: Query<&mut MeshMaterial2d<ColorMaterial>>,
    card_materials: Res<AircraftCardDisplayMaterials>,
    selected_aircraft_card_display: Option<Res<SelectedAircraftCardDisplay>>,
    mut commands: Commands,
    mut q_camera: Single<&mut CameraScrollEnabled, With<Camera2d>>,
) {
    let Some(selected) = selected_aircraft_card_display else {
        return;
    };
    let SelectedAircraftCardDisplay { display_entity, .. } = *selected;
    commands.remove_resource::<SelectedAircraftCardDisplay>();
    if let Ok(mut display_material) = q_display.get_mut(display_entity) {
        display_material.0 = card_materials.normal.clone();
    }
    q_camera.0 = true;
}
