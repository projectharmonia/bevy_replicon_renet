use bevy::prelude::*;
use bevy_renet::{self, renet::RenetClient, RenetClientPlugin, RenetReceive, RenetSend};
#[cfg(feature = "renet_transport")]
use bevy_renet::{renet::transport::NetcodeClientTransport, transport::NetcodeClientPlugin};
use bevy_replicon::prelude::*;

use crate::ClientIdExt;

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
                    Self::set_connecting.run_if(bevy_renet::client_connecting),
                    Self::set_disconnected.run_if(bevy_renet::client_just_disconnected),
                    Self::set_connected.run_if(bevy_renet::client_just_connected),
                    Self::receive_packets.run_if(bevy_renet::client_connected),
                )
                    .chain()
                    .in_set(ClientSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                Self::send_packets
                    .in_set(ClientSet::SendPackets)
                    .run_if(bevy_renet::client_connected),
            );

        #[cfg(feature = "renet_transport")]
        app.add_plugins(NetcodeClientPlugin);
    }
}

impl RepliconRenetClientPlugin {
    fn set_disconnected(mut client: ResMut<RepliconClient>) {
        client.set_status(RepliconClientStatus::Disconnected);
    }

    fn set_connecting(mut client: ResMut<RepliconClient>) {
        if client.status() != RepliconClientStatus::Connecting {
            client.set_status(RepliconClientStatus::Connecting);
        }
    }

    fn set_connected(
        mut client: ResMut<RepliconClient>,
        #[cfg(feature = "renet_transport")] transport: Res<NetcodeClientTransport>,
    ) {
        // In renet only transport knows the ID.
        // TODO: Pending renet issue https://github.com/lucaspoffo/renet/issues/153
        #[cfg(feature = "renet_transport")]
        let client_id = Some(transport.client_id().as_client_id());
        #[cfg(not(feature = "renet_transport"))]
        let client_id = None;

        client.set_status(RepliconClientStatus::Connected { client_id });
    }

    fn receive_packets(
        channels: Res<RepliconChannels>,
        mut renet_client: ResMut<RenetClient>,
        mut replicon_client: ResMut<RepliconClient>,
    ) {
        for channel_id in 0..channels.server_channels().len() as u8 {
            while let Some(message) = renet_client.receive_message(channel_id) {
                replicon_client.insert_received(channel_id, message);
            }
        }
    }

    fn send_packets(
        mut renet_client: ResMut<RenetClient>,
        mut replicon_client: ResMut<RepliconClient>,
    ) {
        for (channel_id, message) in replicon_client.drain_sent() {
            renet_client.send_message(channel_id, message)
        }
    }
}
