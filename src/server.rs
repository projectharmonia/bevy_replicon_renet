use bevy::prelude::*;
#[cfg(feature = "renet_netcode")]
use bevy_renet::netcode::NetcodeServerPlugin;
#[cfg(feature = "renet_steam")]
use bevy_renet::steam::SteamServerPlugin;
use bevy_renet::{
    renet::{RenetServer, ServerEvent},
    RenetReceive, RenetSend, RenetServerPlugin,
};
use bevy_replicon::{
    core::connected_client::{ClientId, ClientIdMap},
    prelude::*,
};

/// Adds renet as server messaging backend.
///
/// Initializes [`RenetServerPlugin`], systems that pass data between [`RenetServer`]
/// and [`RepliconServer`] and translates renet's server events into replicon's.
pub struct RepliconRenetServerPlugin;

impl Plugin for RepliconRenetServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetServerPlugin)
            .configure_sets(PreUpdate, ServerSet::ReceivePackets.after(RenetReceive))
            .configure_sets(PostUpdate, ServerSet::SendPackets.before(RenetSend))
            .add_systems(
                PreUpdate,
                (
                    Self::set_running.run_if(resource_added::<RenetServer>),
                    Self::set_stopped.run_if(resource_removed::<RenetServer>),
                    (Self::receive_packets, Self::forward_server_events)
                        .run_if(resource_exists::<RenetServer>),
                )
                    .chain()
                    .in_set(ServerSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                Self::send_packets
                    .in_set(ServerSet::SendPackets)
                    .run_if(resource_exists::<RenetServer>),
            );

        #[cfg(feature = "renet_netcode")]
        app.add_plugins(NetcodeServerPlugin);
        #[cfg(feature = "renet_steam")]
        app.add_plugins(SteamServerPlugin);
    }
}

impl RepliconRenetServerPlugin {
    fn set_running(mut server: ResMut<RepliconServer>) {
        server.set_running(true);
    }

    fn set_stopped(mut server: ResMut<RepliconServer>) {
        server.set_running(false);
    }

    fn forward_server_events(
        mut commands: Commands,
        mut server_events: EventReader<ServerEvent>,
        client_map: Res<ClientIdMap>,
    ) {
        for event in server_events.read() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    let client_id = ClientId::new(*client_id);
                    const MAX_SIZE: usize = 1200; // From https://github.com/lucaspoffo/renet/blob/master/renet/src/packet.rs#L7
                    let client_entity = commands
                        .spawn(ConnectedClient::new(client_id, MAX_SIZE))
                        .id();
                    debug!("connecting `{client_entity}` with `{client_id:?}`");
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    let client_id = ClientId::new(*client_id);
                    let client_entity = *client_map
                        .get(&client_id)
                        .expect("clients should be connected before disconnection");

                    commands.entity(client_entity).despawn();
                    debug!("disconnecting `{client_entity}` with `{client_id:?}`: {reason}");
                }
            }
        }
    }

    fn receive_packets(
        channels: Res<RepliconChannels>,
        mut renet_server: ResMut<RenetServer>,
        mut replicon_server: ResMut<RepliconServer>,
        mut clients: Query<(Entity, &ConnectedClient, &mut NetworkStats)>,
    ) {
        for (client_entity, client, mut stats) in &mut clients {
            for channel_id in 0..channels.client_channels().len() as u8 {
                while let Some(message) =
                    renet_server.receive_message(client.id().get(), channel_id)
                {
                    trace!(
                        "forwarding {} received bytes over channel {channel_id}",
                        message.len()
                    );
                    replicon_server.insert_received(client_entity, channel_id, message);
                }
            }

            // Renet events reading runs in parallel, so the client might have been disconnected.
            if let Ok(info) = renet_server.network_info(client.id().get()) {
                stats.rtt = info.rtt;
                stats.packet_loss = info.packet_loss;
                stats.sent_bps = info.bytes_sent_per_second;
                stats.received_bps = info.bytes_received_per_second;
            }
        }
    }

    fn send_packets(
        mut renet_server: ResMut<RenetServer>,
        mut replicon_server: ResMut<RepliconServer>,
        clients: Query<&ConnectedClient>,
    ) {
        for (client_entity, channel_id, message) in replicon_server.drain_sent() {
            trace!(
                "forwarding {} sent bytes over channel {channel_id}",
                message.len()
            );
            let client = clients
                .get(client_entity)
                .expect("messages should be sent only to connected clients");
            renet_server.send_message(client.id().get(), channel_id, message)
        }
    }
}
