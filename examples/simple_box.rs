//! A simple demo to showcase how player could send inputs to move the square and server replicates position back.
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
        app.replicate::<PlayerPosition>()
            .replicate::<PlayerColor>()
            .add_client_event::<MoveDirection>(ChannelKind::Ordered)
            .add_systems(
                Startup,
                (Self::read_cli.map(Result::unwrap), Self::spawn_camera),
            )
            .add_systems(
                Update,
                (
                    Self::apply_movement.run_if(server_or_singleplayer),
                    Self::handle_connections.run_if(server_running),
                    (Self::draw_boxes, Self::read_input),
                ),
            );
    }
}

impl SimpleBoxPlugin {
    fn read_cli(
        mut commands: Commands,
        cli: Res<Cli>,
        channels: Res<RepliconChannels>,
    ) -> Result<(), Box<dyn Error>> {
        match *cli {
            Cli::SinglePlayer => {
                commands.spawn((Player(ClientId::SERVER), PlayerColor(GREEN.into())));
            }
            Cli::Server { port } => {
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
                commands.spawn((Player(ClientId::SERVER), PlayerColor(GREEN.into())));
            }
            Cli::Client { port, ip } => {
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

    /// Logs server events and spawns a new player whenever a client connects.
    fn handle_connections(mut commands: Commands, mut server_events: EventReader<ServerEvent>) {
        for event in server_events.read() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    info!("{client_id:?} connected");
                    // Generate pseudo random color from client id.
                    let r = ((client_id.get() % 23) as f32) / 23.0;
                    let g = ((client_id.get() % 27) as f32) / 27.0;
                    let b = ((client_id.get() % 39) as f32) / 39.0;
                    commands.spawn((Player(*client_id), PlayerColor(Color::srgb(r, g, b))));
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    info!("{client_id:?} disconnected: {reason}");
                }
            }
        }
    }

    fn draw_boxes(mut gizmos: Gizmos, players: Query<(&PlayerPosition, &PlayerColor)>) {
        for (position, color) in &players {
            gizmos.rect(
                Vec3::new(position.x, position.y, 0.0),
                Vec2::ONE * 50.0,
                color.0,
            );
        }
    }

    /// Reads player inputs and sends [`MoveDirection`] events.
    fn read_input(mut move_events: EventWriter<MoveDirection>, input: Res<ButtonInput<KeyCode>>) {
        let mut direction = Vec2::ZERO;
        if input.pressed(KeyCode::ArrowRight) {
            direction.x += 1.0;
        }
        if input.pressed(KeyCode::ArrowLeft) {
            direction.x -= 1.0;
        }
        if input.pressed(KeyCode::ArrowUp) {
            direction.y += 1.0;
        }
        if input.pressed(KeyCode::ArrowDown) {
            direction.y -= 1.0;
        }
        if direction != Vec2::ZERO {
            move_events.send(MoveDirection(direction.normalize_or_zero()));
        }
    }

    /// Mutates [`PlayerPosition`] based on [`MoveDirection`] events.
    ///
    /// Fast-paced games usually you don't want to wait until server send a position back because of the latency.
    /// But this example just demonstrates simple replication concept.
    fn apply_movement(
        time: Res<Time>,
        mut move_events: EventReader<FromClient<MoveDirection>>,
        mut players: Query<(&Player, &mut PlayerPosition)>,
    ) {
        const MOVE_SPEED: f32 = 300.0;
        for FromClient { client_id, event } in move_events.read() {
            info!("received event {event:?} from {client_id:?}");
            for (player, mut position) in &mut players {
                if *client_id == player.0 {
                    **position += event.0 * time.delta_secs() * MOVE_SPEED;
                }
            }
        }
    }
}

const PORT: u16 = 5000;
const PROTOCOL_ID: u64 = 0;

#[derive(Parser, PartialEq, Resource)]
enum Cli {
    SinglePlayer,
    Server {
        #[arg(short, long, default_value_t = PORT)]
        port: u16,
    },
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

/// Contains player ID.
#[derive(Component, Serialize, Deserialize)]
#[require(PlayerPosition, PlayerColor, Replicated)]
struct Player(ClientId);

#[derive(Component, Deserialize, Serialize, Deref, DerefMut, Default)]
struct PlayerPosition(Vec2);

#[derive(Component, Deserialize, Serialize, Default)]
struct PlayerColor(Color);

/// A movement event for the controlled box.
#[derive(Debug, Default, Deserialize, Event, Serialize)]
struct MoveDirection(Vec2);
