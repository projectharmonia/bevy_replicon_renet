use bevy::prelude::*;
#[cfg(feature = "renet_netcode")]
use bevy_renet::netcode::NetcodeClientPlugin;
#[cfg(feature = "renet_steam")]
use bevy_renet::steam::SteamClientPlugin;
use bevy_renet::{self, RenetClientPlugin, RenetReceive, RenetSend, renet::RenetClient};
use bevy_replicon::prelude::*;

/// Adds renet as client messaging backend.
///
/// Initializes [`RenetClientPlugin`] and systems that pass data between
/// [`RenetClient`] and [`RepliconClient`].
pub struct RepliconRenetClientPlugin;

impl Plugin for RepliconRenetClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetClientPlugin)
            .configure_sets(PreUpdate, ClientSet::ReceivePackets.after(RenetReceive))
            .configure_sets(PostUpdate, ClientSet::SendPackets.before(RenetSend))
            .add_systems(
                PreUpdate,
                (
                    set_connecting.run_if(bevy_renet::client_connecting),
                    set_disconnected.run_if(bevy_renet::client_just_disconnected),
                    set_connected.run_if(bevy_renet::client_just_connected),
                    receive_packets.run_if(bevy_renet::client_connected),
                )
                    .chain()
                    .in_set(ClientSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                send_packets
                    .in_set(ClientSet::SendPackets)
                    .run_if(bevy_renet::client_connected),
            );

        #[cfg(feature = "renet_netcode")]
        app.add_plugins(NetcodeClientPlugin);
        #[cfg(feature = "renet_steam")]
        app.add_plugins(SteamClientPlugin);
    }
}

fn set_disconnected(mut client: ResMut<RepliconClient>) {
    client.set_status(RepliconClientStatus::Disconnected);
}

fn set_connecting(mut client: ResMut<RepliconClient>) {
    if client.status() != RepliconClientStatus::Connecting {
        client.set_status(RepliconClientStatus::Connecting);
    }
}

fn set_connected(mut client: ResMut<RepliconClient>) {
    client.set_status(RepliconClientStatus::Connected);
}

fn receive_packets(
    channels: Res<RepliconChannels>,
    mut renet_client: ResMut<RenetClient>,
    mut replicon_client: ResMut<RepliconClient>,
) {
    for channel_id in 0..channels.server_channels().len() as u8 {
        while let Some(message) = renet_client.receive_message(channel_id) {
            trace!(
                "forwarding {} received bytes over channel {channel_id}",
                message.len()
            );
            replicon_client.insert_received(channel_id, message);
        }
    }

    let stats = replicon_client.stats_mut();
    stats.rtt = renet_client.rtt();
    stats.packet_loss = renet_client.packet_loss();
    stats.sent_bps = renet_client.bytes_sent_per_sec();
    stats.received_bps = renet_client.bytes_received_per_sec();
}

fn send_packets(
    mut renet_client: ResMut<RenetClient>,
    mut replicon_client: ResMut<RepliconClient>,
) {
    for (channel_id, message) in replicon_client.drain_sent() {
        trace!(
            "forwarding {} sent bytes over channel {channel_id}",
            message.len()
        );
        renet_client.send_message(channel_id as u8, message)
    }
}
