/*!
Provides integration for [`bevy_replicon`](https://docs.rs/bevy_replicon) for [`bevy_renet`](https://docs.rs/bevy_renet).

# Getting started

This guide assumes that you have already read [quick start guide](https://docs.rs/bevy_replicon#quick-start) from `bevy_replicon`.

All Renet API is re-exported from this plugin, you don't need to include `bevy_renet` or `renet` to your `Cargo.toml`.

Renet by default uses the netcode transport which is re-exported by the `renet_transport` feature. If you want to use other transports, you can disable it.

## Initialization

Add [`RepliconRenetPlugins`] along with [`RepliconPlugins`]:

```
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;

let mut app = App::new();
app.add_plugins((MinimalPlugins, RepliconPlugins, RepliconRenetPlugins));
```
If you want to separate the client and server, you can use the `client` and `server` features (both enabled by default),
which control enabled plugins. These features automatically enable corresponding features in `bevy_replicon`.

It's also possible to do it at runtime via [`PluginGroupBuilder::disable()`].
For server disable [`RepliconRenetClientPlugin`].
For client disable [`RepliconRenetServerPlugin`].

Plugins in [`RepliconRenetPlugins`] automatically add `renet` plugins, you don't need to add them.
If the `renet_transport` feature is enabled, netcode plugins will also be automatically added.

## Server and client creation

To connect to the server or create it, you need to initialize the
[`RenetClient`](renet::RenetClient) and [`NetcodeClientTransport`](renet::transport::NetcodeClientTransport) **or**
[`RenetServer`](renet::RenetServer) and [`NetcodeServerTransport`](renet::transport::NetcodeServerTransport)
resources from Renet.

Never insert client and server resources in the same app for single-player, it will cause a replication loop.

This crate provides the [`RenetChannelsExt`] extension trait to conveniently convert channels
from the [`RepliconChannels`] resource into renet channels.
When creating a server or client you need to use a [`ConnectionConfig`](renet::ConnectionConfig)
from [`renet`], which can be initialized like this:

```
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{renet::ConnectionConfig, RenetChannelsExt, RepliconRenetPlugins};

# let mut app = App::new();
# app.add_plugins(RepliconPlugins);
let channels = app.world().resource::<RepliconChannels>();
let connection_config = ConnectionConfig {
    server_channels_config: channels.get_server_configs(),
    client_channels_config: channels.get_client_configs(),
    ..Default::default()
};
```

For a full example of how to initialize a server or client see the example in the
repository.
*/
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

pub use bevy_renet::renet;
#[cfg(feature = "renet_transport")]
pub use bevy_renet::transport;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_replicon::prelude::*;
use renet::{ChannelConfig, SendType};

#[cfg(feature = "client")]
use client::RepliconRenetClientPlugin;
#[cfg(feature = "server")]
use server::RepliconRenetServerPlugin;

/// Plugin group for all replicon plugins for renet.
///
/// Contains the following:
/// * [`RepliconRenetServerPlugin`] - with feature `server`.
/// * [`RepliconRenetClientPlugin`] - with feature `client`.
pub struct RepliconRenetPlugins;

impl PluginGroup for RepliconRenetPlugins {
    fn build(self) -> PluginGroupBuilder {
        let mut group = PluginGroupBuilder::start::<Self>();

        #[cfg(feature = "server")]
        {
            group = group.add(RepliconRenetServerPlugin);
        }

        #[cfg(feature = "client")]
        {
            group = group.add(RepliconRenetClientPlugin);
        }

        group
    }
}

/// External trait for [`RepliconChannels`] to provide convenient conversion into renet channel configs.
pub trait RenetChannelsExt {
    /// Returns server channel configs that can be used to create [`ConnectionConfig`](renet::ConnectionConfig).
    fn get_server_configs(&self) -> Vec<ChannelConfig>;

    /// Same as [`RenetChannelsExt::get_server_configs`], but for clients.
    fn get_client_configs(&self) -> Vec<ChannelConfig>;
}

impl RenetChannelsExt for RepliconChannels {
    fn get_server_configs(&self) -> Vec<ChannelConfig> {
        create_configs(self.server_channels(), self.default_max_bytes)
    }

    fn get_client_configs(&self) -> Vec<ChannelConfig> {
        create_configs(self.client_channels(), self.default_max_bytes)
    }
}

/// Converts replicon channels into renet channel configs.
fn create_configs(channels: &[RepliconChannel], default_max_bytes: usize) -> Vec<ChannelConfig> {
    let mut channel_configs = Vec::with_capacity(channels.len());
    for (index, channel) in channels.iter().enumerate() {
        let send_type = match channel.kind {
            ChannelKind::Unreliable => SendType::Unreliable,
            ChannelKind::Unordered => SendType::ReliableUnordered {
                resend_time: channel.resend_time,
            },
            ChannelKind::Ordered => SendType::ReliableOrdered {
                resend_time: channel.resend_time,
            },
        };
        channel_configs.push(ChannelConfig {
            channel_id: index as u8,
            max_memory_usage_bytes: channel.max_bytes.unwrap_or(default_max_bytes),
            send_type,
        });
    }
    channel_configs
}
