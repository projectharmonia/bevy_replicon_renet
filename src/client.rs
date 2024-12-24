use bevy::prelude::*;
#[cfg(feature = "renet_netcode")]
use bevy_renet::netcode::{NetcodeClientPlugin, NetcodeClientTransport};
#[cfg(feature = "renet_steam")]
use bevy_renet::netcode::{SteamClientPlugin, SteamClientTransport};
use bevy_renet::{self, renet::RenetClient, RenetClientPlugin, RenetReceive, RenetSend};
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

        #[cfg(feature = "renet_netcode")]
        app.add_plugins(NetcodeClientPlugin);
        #[cfg(feature = "renet_steam")]
        app.add_plugins(SteamClientPlugin);
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
        #[cfg(feature = "renet_netcode")] netcode_transport: Option<Res<NetcodeClientTransport>>,
        #[cfg(feature = "renet_steam")] steam_transport: Option<Res<SteamClientTransport>>,
    ) {
        // In renet only transport knows the ID.
        // TODO: Pending renet issue https://github.com/lucaspoffo/renet/issues/153
        #[allow(unused_mut)]
        let mut client_id = None;
        #[cfg(feature = "renet_netcode")]
        if let Some(transport) = netcode_transport {
            client_id = Some(ClientId::new(transport.client_id()));
        }
        #[cfg(feature = "renet_steam")]
        if let Some(transport) = steam_transport {
            assert!(
                client_id.is_none(),
                "two transports can't be active at the same time",
            );
            client_id = Some(ClientId::new(transport.client_id()));
        }

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
