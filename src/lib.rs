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
[`RenetClient`](renet::RenetClient) and [`NetcodeClientTransport`](bevy_renet::netcode::NetcodeClientTransport) **or**
[`RenetServer`](renet::RenetServer) and [`NetcodeServerTransport`](bevy_renet::netcode::NetcodeServerTransport)
resources from Renet.

For steam transport you need to activate the corresponding and use its transport resource instead.

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

## Replicon conditions

The crate updates the running state of [`RepliconServer`] and connection state of [`RepliconClient`]
based on the states of [`RenetServer`](renet::RenetServer) and [`RenetClient`](renet::RenetServer)
in [`PreUpdate`].

This means that [replicon conditions](bevy_replicon::core::common_conditions) won't work in schedules
like [`Startup`]. As a workaround, you can directly check if renet's resources are present. This may be resolved
in the future once we have [observers for resources](https://github.com/bevyengine/bevy/issues/12231)
to immediately react to changes.
*/
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "renet_netcode")]
pub use bevy_renet::netcode;
pub use bevy_renet::renet;
#[cfg(feature = "renet_steam")]
pub use bevy_renet::steam;

#[cfg(feature = "client")]
pub use client::RepliconRenetClientPlugin;
#[cfg(feature = "server")]
pub use server::RepliconRenetServerPlugin;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_replicon::prelude::*;
use renet::{ChannelConfig, SendType};

/// Plugin group for all replicon renet backend plugins.
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
        let config = ChannelConfig {
            channel_id: index as u8,
            max_memory_usage_bytes: channel.max_bytes.unwrap_or(default_max_bytes),
            send_type,
        };

        debug!("creating channel config `{config:?}`");
        channel_configs.push(config);
    }
    channel_configs
}
