//! A simple demo to showcase how player could send inputs to move a box and server replicates position back.
//! Also demonstrates the single-player and how sever also could be a player.

use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::{
    color::palettes::css::GREEN,
    prelude::*,
    winit::{UpdateMode::Continuous, WinitSettings},
};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{
        ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
        ServerConfig,
    },
    renet::{ConnectionConfig, RenetClient, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use clap::Parser;
use serde::{Deserialize, Serialize};

fn main() {
    App::new()
        .init_resource::<Cli>() // Parse CLI before creating window.
        // Makes the server/client update continuously even while unfocused.
        .insert_resource(WinitSettings {
            focused_mode: Continuous,
            unfocused_mode: Continuous,
        })
        .add_plugins((
            DefaultPlugins,
            RepliconPlugins,
            RepliconRenetPlugins,
            SimpleBoxPlugin,
        ))
        .run();
}

struct SimpleBoxPlugin;

impl Plugin for SimpleBoxPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<BoxPosition>()
            .replicate::<BoxColor>()
            .add_client_trigger::<MoveBox>(ChannelKind::Ordered)
            .add_observer(spawn_clients)
            .add_observer(despawn_clients)
            .add_observer(apply_movement)
            .add_systems(Startup, (read_cli.map(Result::unwrap), spawn_camera))
            .add_systems(Update, (read_input, draw_boxes));
    }
}

fn read_cli(
    mut commands: Commands,
    cli: Res<Cli>,
    channels: Res<RepliconChannels>,
) -> Result<(), Box<dyn Error>> {
    const PROTOCOL_ID: u64 = 0;

    match *cli {
        Cli::SinglePlayer => {
            info!("starting single-player game");
            commands.spawn((BoxPlayer(ClientId::SERVER), BoxColor(GREEN.into())));
        }
        Cli::Server { port } => {
            info!("starting server at port {port}");
            let server_channels_config = channels.get_server_configs();
            let client_channels_config = channels.get_client_configs();

            let server = RenetServer::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
            let server_config = ServerConfig {
                current_time,
                max_clients: 10,
                protocol_id: PROTOCOL_ID,
                authentication: ServerAuthentication::Unsecure,
                public_addresses: Default::default(),
            };
            let transport = NetcodeServerTransport::new(server_config, socket)?;

            commands.insert_resource(server);
            commands.insert_resource(transport);

            commands.spawn((
                Text::new("Server"),
                TextFont {
                    font_size: 30.0,
                    ..Default::default()
                },
                TextColor::WHITE,
            ));
            commands.spawn((BoxPlayer(ClientId::SERVER), BoxColor(GREEN.into())));
        }
        Cli::Client { port, ip } => {
            info!("connecting to {ip}:{port}");
            let server_channels_config = channels.get_server_configs();
            let client_channels_config = channels.get_client_configs();

            let client = RenetClient::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let client_id = current_time.as_millis() as u64;
            let server_addr = SocketAddr::new(ip, port);
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
            let authentication = ClientAuthentication::Unsecure {
                client_id,
                protocol_id: PROTOCOL_ID,
                server_addr,
                user_data: None,
            };
            let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

            commands.insert_resource(client);
            commands.insert_resource(transport);

            commands.spawn((
                Text(format!("Client: {client_id:?}")),
                TextFont {
                    font_size: 30.0,
                    ..default()
                },
                TextColor::WHITE,
            ));
        }
    }

    Ok(())
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Spawns a new box whenever a client connects.
fn spawn_clients(trigger: Trigger<ClientConnected>, mut commands: Commands) {
    // Generate pseudo random color from client id.
    let r = ((trigger.client_id.get() % 23) as f32) / 23.0;
    let g = ((trigger.client_id.get() % 27) as f32) / 27.0;
    let b = ((trigger.client_id.get() % 39) as f32) / 39.0;
    info!("spawning box for `{:?}`", trigger.client_id);
    commands.spawn((BoxPlayer(trigger.client_id), BoxColor(Color::srgb(r, g, b))));
}

/// Despawns a box whenever a client disconnects.
fn despawn_clients(
    trigger: Trigger<ClientDisconnected>,
    mut commands: Commands,
    boxes: Query<(Entity, &BoxPlayer)>,
) {
    let (entity, _) = boxes
        .iter()
        .find(|(_, &player)| *player == trigger.client_id)
        .expect("all clients should have entities");
    commands.entity(entity).despawn();
}

/// Reads player inputs and sends [`MoveDirection`] events.
fn read_input(mut commands: Commands, input: Res<ButtonInput<KeyCode>>) {
    let mut direction = Vec2::ZERO;
    if input.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction != Vec2::ZERO {
        commands.client_trigger(MoveBox(direction.normalize_or_zero()));
    }
}

/// Mutates [`BoxPosition`] based on [`MoveBox`] events.
///
/// Fast-paced games usually you don't want to wait until server send a position back because of the latency.
/// But this example just demonstrates simple replication concept.
fn apply_movement(
    trigger: Trigger<FromClient<MoveBox>>,
    time: Res<Time>,
    mut boxes: Query<(&BoxPlayer, &mut BoxPosition)>,
) {
    const MOVE_SPEED: f32 = 300.0;
    info!("received movement from `{:?}`", trigger.client_id);
    for (player, mut position) in &mut boxes {
        // Find the sender entity. We don't include the entity as a trigger target to save traffic, since the server knows
        // which entity to apply the input to. We could have a resource that maps connected clients to controlled entities,
        // but we didn't implement it for the sake of simplicity.
        if trigger.client_id == **player {
            **position += *trigger.event * time.delta_secs() * MOVE_SPEED;
        }
    }
}

fn draw_boxes(mut gizmos: Gizmos, boxes: Query<(&BoxPosition, &BoxColor)>) {
    for (position, color) in &boxes {
        gizmos.rect(
            Vec3::new(position.x, position.y, 0.0),
            Vec2::ONE * 50.0,
            **color,
        );
    }
}

const PORT: u16 = 5000;

/// A simple game with moving boxes.
#[derive(Parser, PartialEq, Resource)]
enum Cli {
    /// No networking will be used, the player will control its box locally.
    SinglePlayer,
    /// Run game instance will act as both a player and a host.
    Server {
        #[arg(short, long, default_value_t = PORT)]
        port: u16,
    },
    /// The game instance will connect to a host in order to start the game.
    Client {
        #[arg(short, long, default_value_t = Ipv4Addr::LOCALHOST.into())]
        ip: IpAddr,

        #[arg(short, long, default_value_t = PORT)]
        port: u16,
    },
}

impl Default for Cli {
    fn default() -> Self {
        Self::parse()
    }
}

/// Identifies which player controls the box.
///
/// We want to replicate all boxes, so we just set [`Replicated`] as a required component.
#[derive(Component, Clone, Copy, Deref, Serialize, Deserialize)]
#[require(BoxPosition, BoxColor, Replicated)]
struct BoxPlayer(ClientId);

#[derive(Component, Deserialize, Serialize, Deref, DerefMut, Default)]
struct BoxPosition(Vec2);

#[derive(Component, Deref, Deserialize, Serialize, Default)]
struct BoxColor(Color);

/// A movement event for the controlled box.
#[derive(Deserialize, Deref, Event, Serialize)]
struct MoveBox(Vec2);
