//! A game to showcase single-player and multiplier game.
//! Run it with `--hotseat` to play locally or with `--client` / `--server`

use std::{
    error::Error,
    fmt::{self, Formatter},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{
        ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
        ServerConfig,
    },
    renet::{ConnectionConfig, RenetClient, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

fn main() {
    App::new()
        .init_resource::<Cli>() // Parse CLI before creating window.
        .add_plugins((
            DefaultPlugins.build().set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Tic-Tac-Toe".into(),
                    resolution: (800.0, 600.0).into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            RepliconPlugins,
            RepliconRenetPlugins,
            TicTacToePlugin,
        ))
        .run();
}

struct TicTacToePlugin;

impl Plugin for TicTacToePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<SymbolFont>()
            .init_resource::<CurrentTurn>()
            .replicate::<Symbol>()
            .replicate::<CellIndex>()
            .replicate::<Player>()
            .add_client_event::<CellPick>(ChannelKind::Ordered)
            .insert_resource(ClearColor(BACKGROUND_COLOR))
            .add_systems(
                Startup,
                (Self::setup_ui, Self::read_cli.map(Result::unwrap)),
            )
            .add_systems(
                OnEnter(GameState::InGame),
                (Self::show_turn_text, Self::show_turn_symbol),
            )
            .add_systems(
                OnEnter(GameState::Disconnected),
                Self::show_disconnected_text,
            )
            .add_systems(OnEnter(GameState::Winner), Self::show_winner_text)
            .add_systems(OnEnter(GameState::Tie), Self::show_tie_text)
            .add_systems(
                Update,
                (
                    Self::show_connecting_text.run_if(resource_added::<RenetClient>),
                    Self::show_waiting_player_text.run_if(resource_added::<RenetServer>),
                    Self::handle_connections.run_if(server_running),
                    Self::start_game
                        .run_if(client_connected)
                        .run_if(any_component_added::<Player>), // Wait until client replicates players before starting the game.
                    (
                        Self::handle_interactions.run_if(local_player_turn),
                        Self::spawn_symbols.run_if(server_or_singleplayer),
                        Self::init_symbols,
                        Self::advance_turn.run_if(any_component_added::<CellIndex>),
                        Self::show_turn_symbol.run_if(resource_changed::<CurrentTurn>),
                    )
                        .run_if(in_state(GameState::InGame)),
                ),
            );
    }
}

const GRID_SIZE: usize = 3;

const BACKGROUND_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

const PROTOCOL_ID: u64 = 0;

// Bottom text defined in two sections, first for text and second for symbols with different font.
const TEXT_SECTION: usize = 0;
const SYMBOL_SECTION: usize = 1;

impl TicTacToePlugin {
    fn setup_ui(mut commands: Commands, symbol_font: Res<SymbolFont>) {
        commands.spawn(Camera2d);

        const LINES_COUNT: usize = GRID_SIZE + 1;

        const CELL_SIZE: f32 = 100.0;
        const LINE_THICKNESS: f32 = 10.0;
        const BOARD_SIZE: f32 = CELL_SIZE * GRID_SIZE as f32 + LINES_COUNT as f32 * LINE_THICKNESS;

        const BOARD_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);

        for line in 0..LINES_COUNT {
            let position = -BOARD_SIZE / 2.0
                + line as f32 * (CELL_SIZE + LINE_THICKNESS)
                + LINE_THICKNESS / 2.0;

            // Horizontal
            commands.spawn((
                Sprite {
                    color: BOARD_COLOR,
                    ..Default::default()
                },
                Transform {
                    translation: Vec3::Y * position,
                    scale: Vec3::new(BOARD_SIZE, LINE_THICKNESS, 1.0),
                    ..Default::default()
                },
            ));

            // Vertical
            commands.spawn((
                Sprite {
                    color: BOARD_COLOR,
                    ..Default::default()
                },
                Transform {
                    translation: Vec3::X * position,
                    scale: Vec3::new(LINE_THICKNESS, BOARD_SIZE, 1.0),
                    ..Default::default()
                },
            ));
        }

        const BUTTON_SIZE: f32 = CELL_SIZE / 1.2;
        const BUTTON_MARGIN: f32 = (CELL_SIZE + LINE_THICKNESS - BUTTON_SIZE) / 2.0;

        const TEXT_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
        const FONT_SIZE: f32 = 32.0;

        commands
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            })
            .with_children(|parent| {
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(BOARD_SIZE - LINE_THICKNESS),
                        height: Val::Px(BOARD_SIZE - LINE_THICKNESS),
                        ..Default::default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn((
                                GridNode,
                                Node {
                                    display: Display::Grid,
                                    grid_template_columns: vec![GridTrack::auto(); GRID_SIZE],
                                    ..Default::default()
                                },
                            ))
                            .with_children(|parent| {
                                for _ in 0..GRID_SIZE * GRID_SIZE {
                                    parent.spawn((
                                        Button,
                                        Node {
                                            width: Val::Px(BUTTON_SIZE),
                                            height: Val::Px(BUTTON_SIZE),
                                            margin: UiRect::all(Val::Px(BUTTON_MARGIN)),
                                            ..Default::default()
                                        },
                                        BackgroundColor(BACKGROUND_COLOR.into()),
                                    ));
                                }
                            });

                        parent
                            .spawn(Node {
                                margin: UiRect::top(Val::Px(20.0)),
                                justify_content: JustifyContent::Center,
                                ..Default::default()
                            })
                            .with_children(|parent| {
                                parent
                                    .spawn((
                                        Text::default(),
                                        TextFont {
                                            font_size: FONT_SIZE,
                                            ..Default::default()
                                        },
                                        TextColor(TEXT_COLOR),
                                        BottomText,
                                    ))
                                    .with_child((
                                        TextSpan::default(),
                                        TextFont {
                                            font: symbol_font.0.clone(),
                                            font_size: FONT_SIZE,
                                            ..Default::default()
                                        },
                                        TextColor(TEXT_COLOR),
                                    ));
                            });
                    });
            });
    }

    fn read_cli(
        mut commands: Commands,
        mut game_state: ResMut<NextState<GameState>>,
        cli: Res<Cli>,
        channels: Res<RepliconChannels>,
    ) -> Result<(), Box<dyn Error>> {
        match *cli {
            Cli::Hotseat => {
                // Set all players to server to play from a single machine and start the game right away.
                commands.spawn((Player(ClientId::SERVER), Symbol::Cross));
                commands.spawn((Player(ClientId::SERVER), Symbol::Nought));
                game_state.set(GameState::InGame);
            }
            Cli::Server { port, symbol } => {
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
                    max_clients: 1,
                    protocol_id: PROTOCOL_ID,
                    authentication: ServerAuthentication::Unsecure,
                    public_addresses: Default::default(),
                };
                let transport = NetcodeServerTransport::new(server_config, socket)?;

                commands.insert_resource(server);
                commands.insert_resource(transport);
                commands.spawn((Player(ClientId::SERVER), symbol));
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
            }
        }

        Ok(())
    }

    fn show_turn_text(mut writer: TextUiWriter, bottom_text: Query<Entity, With<BottomText>>) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Current turn: ".into();
    }

    fn show_turn_symbol(
        mut writer: TextUiWriter,
        current_turn: Res<CurrentTurn>,
        bottom_text: Query<Entity, With<BottomText>>,
    ) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, SYMBOL_SECTION) = current_turn.glyph().into();
        *writer.color(text_entity, SYMBOL_SECTION) = current_turn.color().into();
    }

    fn show_disconnected_text(
        mut writer: TextUiWriter,
        bottom_text: Query<Entity, With<BottomText>>,
    ) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Client disconnected".into();
        writer.text(text_entity, SYMBOL_SECTION).clear();
    }

    fn show_winner_text(mut writer: TextUiWriter, bottom_text: Query<Entity, With<BottomText>>) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Winner: ".into();
    }

    fn show_tie_text(mut writer: TextUiWriter, bottom_text: Query<Entity, With<BottomText>>) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Tie".into();
        writer.text(text_entity, SYMBOL_SECTION).clear();
    }

    fn show_connecting_text(
        mut writer: TextUiWriter,
        bottom_text: Query<Entity, With<BottomText>>,
    ) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Connecting".into();
    }

    fn show_waiting_player_text(
        mut writer: TextUiWriter,
        bottom_text: Query<Entity, With<BottomText>>,
    ) {
        let text_entity = bottom_text.single();
        *writer.text(text_entity, TEXT_SECTION) = "Waiting player".into();
    }

    /// Waits for client to connect to start the game or disconnect to finish it.
    ///
    /// Only for server.
    fn handle_connections(
        mut commands: Commands,
        mut server_events: EventReader<ServerEvent>,
        mut game_state: ResMut<NextState<GameState>>,
        players: Query<&Symbol, With<Player>>,
    ) {
        for event in server_events.read() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    let server_symbol = players.single();
                    commands.spawn((Player(*client_id), server_symbol.next()));
                    game_state.set(GameState::InGame);
                }
                ServerEvent::ClientDisconnected { .. } => {
                    game_state.set(GameState::Disconnected);
                }
            }
        }
    }

    fn start_game(mut game_state: ResMut<NextState<GameState>>) {
        game_state.set(GameState::InGame);
    }

    fn handle_interactions(
        mut buttons: Query<
            (Entity, &Parent, &Interaction, &mut BackgroundColor),
            Changed<Interaction>,
        >,
        children: Query<&Children>,
        mut pick_events: EventWriter<CellPick>,
    ) {
        const HOVER_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);

        for (button_entity, button_parent, interaction, mut background) in &mut buttons {
            match interaction {
                Interaction::Pressed => {
                    let buttons = children.get(**button_parent).unwrap();
                    let index = buttons
                        .iter()
                        .position(|&entity| entity == button_entity)
                        .unwrap();

                    // We send a pick event and wait for the pick to be replicated back to the client.
                    // In case of server or single-player the event will re-translated into [`FromClient`] event to re-use the logic.
                    pick_events.send(CellPick(index));
                }
                Interaction::Hovered => *background = HOVER_COLOR.into(),
                Interaction::None => *background = BACKGROUND_COLOR.into(),
            };
        }
    }

    /// Handles cell pick events.
    ///
    /// Only for single-player and server.
    fn spawn_symbols(
        mut commands: Commands,
        mut pick_events: EventReader<FromClient<CellPick>>,
        symbols: Query<&CellIndex>,
        current_turn: Res<CurrentTurn>,
        players: Query<(&Player, &Symbol)>,
    ) {
        for FromClient { client_id, event } in pick_events.read().copied() {
            // It's good to check the received data, client could be cheating.
            if event.0 > GRID_SIZE * GRID_SIZE {
                debug!("received invalid cell index {:?}", event.0);
                continue;
            }

            if !players
                .iter()
                .any(|(player, &symbol)| player.0 == client_id && symbol == current_turn.0)
            {
                debug!("{client_id:?} chose cell {:?} at wrong turn", event.0);
                continue;
            }

            if symbols.iter().any(|cell_index| cell_index.0 == event.0) {
                debug!(
                    "{client_id:?} has chosen an already occupied cell {:?}",
                    event.0
                );
                continue;
            }

            // Spawn "blueprint" of the cell that client will replicate.
            commands.spawn(SymbolBundle::new(current_turn.0, event.0));
        }
    }

    /// Initializes spawned symbol on client after replication and on server / single-player right after the spawn.
    fn init_symbols(
        mut commands: Commands,
        symbol_font: Res<SymbolFont>,
        symbols: Query<(Entity, &CellIndex, &Symbol), Added<Symbol>>,
        grid_nodes: Query<&Children, With<GridNode>>,
        mut background_colors: Query<&mut BackgroundColor>,
    ) {
        for (symbol_entity, cell_index, symbol) in &symbols {
            let children = grid_nodes.single();
            let button_entity = *children
                .get(cell_index.0)
                .expect("symbols should point to valid buttons");

            let mut background = background_colors
                .get_mut(button_entity)
                .expect("buttons should be initialized with color");
            *background = BACKGROUND_COLOR.into();

            commands
                .entity(button_entity)
                .remove::<Interaction>()
                .add_child(symbol_entity);

            commands.entity(symbol_entity).insert((
                Text::new(symbol.glyph()),
                TextFont {
                    font: symbol_font.0.clone(),
                    font_size: 65.0,
                    ..Default::default()
                },
                TextColor(symbol.color()),
            ));
        }
    }

    /// Checks the winner and advances the turn.
    fn advance_turn(
        mut current_turn: ResMut<CurrentTurn>,
        mut game_state: ResMut<NextState<GameState>>,
        symbols: Query<(&CellIndex, &Symbol)>,
    ) {
        let mut board = [None; GRID_SIZE * GRID_SIZE];
        for (cell_index, &symbol) in &symbols {
            board[cell_index.0] = Some(symbol);
        }

        const WIN_CONDITIONS: [[usize; GRID_SIZE]; 8] = [
            [0, 1, 2],
            [3, 4, 5],
            [6, 7, 8],
            [0, 3, 6],
            [1, 4, 7],
            [2, 5, 8],
            [0, 4, 8],
            [2, 4, 6],
        ];

        for indexes in WIN_CONDITIONS {
            let symbols = indexes.map(|index| board[index]);
            if symbols[0].is_some() && symbols.windows(2).all(|symbols| symbols[0] == symbols[1]) {
                game_state.set(GameState::Winner);
                return;
            }
        }

        if board.iter().all(Option::is_some) {
            game_state.set(GameState::Tie);
        } else {
            current_turn.0 = current_turn.next();
        }
    }
}

/// Returns `true` if the local player can select cells.
fn local_player_turn(
    current_turn: Res<CurrentTurn>,
    client: Res<RepliconClient>,
    players: Query<(&Player, &Symbol)>,
) -> bool {
    let client_id = client.id().unwrap_or(ClientId::SERVER);
    players
        .iter()
        .any(|(player, &symbol)| player.0 == client_id && symbol == current_turn.0)
}

/// A condition for systems to check if any component of type `T` was added to the world.
fn any_component_added<T: Component>(components: Query<(), Added<T>>) -> bool {
    !components.is_empty()
}

const PORT: u16 = 5000;

#[derive(Parser, PartialEq, Resource)]
enum Cli {
    Hotseat,
    Server {
        #[arg(short, long, default_value_t = PORT)]
        port: u16,

        #[arg(short, long, default_value_t = Symbol::Cross)]
        symbol: Symbol,
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

/// Font to display unicode characters for [`Symbol`].
#[derive(Resource)]
struct SymbolFont(Handle<Font>);

impl FromWorld for SymbolFont {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("NotoEmoji-Regular.ttf"))
    }
}

#[derive(States, Clone, Copy, Debug, Eq, Hash, PartialEq, Default)]
enum GameState {
    #[default]
    WaitingPlayer,
    InGame,
    Winner,
    Tie,
    Disconnected,
}

/// Contains symbol to be used this turn.
#[derive(Resource, Default, Deref)]
struct CurrentTurn(Symbol);

/// A component that defines the symbol of a player or a filled cell.
#[derive(Clone, Component, Copy, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
enum Symbol {
    #[default]
    Cross,
    Nought,
}

impl Symbol {
    fn glyph(self) -> &'static str {
        match self {
            Symbol::Cross => "❌",
            Symbol::Nought => "⭕",
        }
    }

    fn color(self) -> Color {
        match self {
            Symbol::Cross => Color::srgb(1.0, 0.5, 0.5),
            Symbol::Nought => Color::srgb(0.5, 0.5, 1.0),
        }
    }

    fn next(self) -> Self {
        match self {
            Symbol::Cross => Symbol::Nought,
            Symbol::Nought => Symbol::Cross,
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Symbol::Cross => f.write_str("cross"),
            Symbol::Nought => f.write_str("nought"),
        }
    }
}

/// Marker for UI node with bottom text.
#[derive(Component)]
struct BottomText;

/// Marker for UI node with cells.
#[derive(Component)]
struct GridNode;

#[derive(Bundle)]
struct SymbolBundle {
    symbol: Symbol,
    cell_index: CellIndex,
    replicated: Replicated,
}

impl SymbolBundle {
    fn new(symbol: Symbol, index: usize) -> Self {
        Self {
            cell_index: CellIndex(index),
            symbol,
            replicated: Replicated,
        }
    }
}

/// Marks that the entity is a cell and contains its location in grid.
#[derive(Component, Deserialize, Serialize)]
struct CellIndex(usize);

/// Contains player ID.
#[derive(Component, Serialize, Deserialize)]
#[require(Symbol, Replicated)]
struct Player(ClientId);

/// An event that indicates a symbol pick.
///
/// We don't replicate the whole UI, so we can't just send the picked entity because on server it may be different.
/// So we send the cell location in grid and calculate the entity on server based on this.
#[derive(Clone, Copy, Deserialize, Event, Serialize)]
struct CellPick(usize);
