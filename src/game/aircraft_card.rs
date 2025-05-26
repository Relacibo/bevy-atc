use bevy::text::FontSmoothing;
use bevy::{ecs::component::Component, log::tracing_subscriber::field::display};
use bevy::{prelude::*, text};
use strum::{EnumIter, IntoEnumIterator};

use super::GameState;
use super::{
    AircraftJustSpawned,
    aircraft::{Aircraft, AircraftPhysics},
    heading::Heading,
};

const AIRCRAFT_CARD_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.3,
    blue: 0.1,
    alpha: 0.3,
};

#[derive(Clone, Debug)]
pub struct AircraftCardPlugin;

impl Plugin for AircraftCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (update_aircraft_card, handle_aircraft_just_spawned)
                .run_if(in_state(GameState::Running)),
        );
    }
}

#[derive(Clone, Debug, Component)]
pub struct AircraftCard;

#[derive(Clone, Debug, Component, EnumIter)]
pub enum AircraftCardDisplay {
    Callsign,
    ClearedHeading,
    Heading,
    ClearedSpeed,
    Speed,
    ClearedAltitude,
    Altitude,
}

pub fn update_aircraft_card(
    q_aircraft: Query<(&Children, &Aircraft, &AircraftPhysics)>,
    q_aircraft_card: Query<&Children, With<AircraftCard>>,
    mut q_card_children: Query<(&AircraftCardDisplay, &mut Text2d)>,
) {
    for (aircraft_children, aircraft, aircraft_physics) in q_aircraft {
        let Some(card_children) = aircraft_children
            .iter()
            .find_map(|c| q_aircraft_card.get(c).ok())
        else {
            continue;
        };
        for card_child in card_children {
            let (display, mut text) = q_card_children.get_mut(*card_child).unwrap();
            text.0 = match display {
                AircraftCardDisplay::Callsign => aircraft.call_sign.clone(),
                AircraftCardDisplay::ClearedHeading => aircraft
                    .cleared_heading
                    .map(display_heading)
                    .unwrap_or_default(),
                AircraftCardDisplay::Heading => display_heading(aircraft_physics.heading),
                AircraftCardDisplay::ClearedSpeed => aircraft
                    .cleared_speed_knots
                    .map(display_speed)
                    .unwrap_or_default(),
                AircraftCardDisplay::Speed => display_speed(aircraft_physics.speed_knots),
                AircraftCardDisplay::ClearedAltitude => aircraft
                    .cleared_altitude_feet
                    .map(display_altitude)
                    .unwrap_or_default(),
                AircraftCardDisplay::Altitude => display_altitude(aircraft_physics.altitude_feet),
            };
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
) {
    for event in events.read() {
        let AircraftJustSpawned(aircraft_entity) = event;
        let mut children = Vec::new();
        for (index, display) in AircraftCardDisplay::iter().enumerate() {
            let component = create_aircraft_card_component(display, 60. - index as f32 * 12.);
            let child = commands.spawn(component).id();
            children.push(child);
        }
        let mut entity = commands.spawn((
            AircraftCard,
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(60., 60.),
            })),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_CARD_COLOR))),
            Transform::from_xyz(0., 80., 0.),
            Visibility::Visible,
        ));
        entity.add_children(&children);
        let entity = entity.id();
        commands.entity(*aircraft_entity).add_child(entity);
    }
}

pub fn create_aircraft_card_component(val: AircraftCardDisplay, position: f32) -> impl Bundle {
    (
        val,
        Text2d::default(),
        TextLayout::new_with_justify(JustifyText::Justified),
        TextFont::from_font_size(100.),
        Visibility::Inherited,
        Transform::from_xyz(0., position, 0.).with_scale(Vec3 { x: 0.1, y: 0.1, z: 1. })        
    )
}
