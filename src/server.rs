use bevy::prelude::*;
#[cfg(feature = "renet_netcode")]
use bevy_renet::netcode::NetcodeServerPlugin;
#[cfg(feature = "renet_steam")]
use bevy_renet::steam::SteamServerPlugin;
use bevy_renet::{
    renet::{self, RenetServer},
    RenetReceive, RenetSend, RenetServerPlugin,
};
use bevy_replicon::prelude::*;

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
                    (
                        Self::set_running.run_if(resource_added::<RenetServer>),
                        Self::set_stopped.run_if(resource_removed::<RenetServer>),
                        Self::receive_packets.run_if(resource_exists::<RenetServer>),
                    )
                        .chain()
                        .in_set(ServerSet::ReceivePackets),
                    Self::forward_server_events.in_set(ServerSet::SendEvents),
                ),
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
        mut renet_server_events: EventReader<renet::ServerEvent>,
        mut server_events: EventWriter<ServerEvent>,
    ) {
        for event in renet_server_events.read() {
            debug!("forwarding event `{event:?}`");
            let replicon_event = match event {
                renet::ServerEvent::ClientConnected { client_id } => ServerEvent::ClientConnected {
                    client_id: ClientId::new(*client_id),
                },
                renet::ServerEvent::ClientDisconnected { client_id, reason } => {
                    ServerEvent::ClientDisconnected {
                        client_id: ClientId::new(*client_id),
                        reason: reason.to_string(),
                    }
                }
            };

            server_events.send(replicon_event);
        }
    }

    fn receive_packets(
        connected_clients: Res<ConnectedClients>,
        channels: Res<RepliconChannels>,
        mut renet_server: ResMut<RenetServer>,
        mut replicon_server: ResMut<RepliconServer>,
    ) {
        for &client in connected_clients.iter() {
            for channel_id in 0..channels.client_channels().len() as u8 {
                while let Some(message) =
                    renet_server.receive_message(client.id().get(), channel_id)
                {
                    trace!(
                        "forwarding {} received bytes over channel {channel_id}",
                        message.len()
                    );
                    replicon_server.insert_received(client.id(), channel_id, message);
                }
            }
        }
    }

    fn send_packets(
        mut renet_server: ResMut<RenetServer>,
        mut replicon_server: ResMut<RepliconServer>,
    ) {
        for (client_id, channel_id, message) in replicon_server.drain_sent() {
            trace!(
                "forwarding {} sent bytes over channel {channel_id}",
                message.len()
            );
            renet_server.send_message(client_id.get(), channel_id, message)
        }
    }
}
