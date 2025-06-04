/*!
Provides integration for [`bevy_replicon`](https://docs.rs/bevy_replicon) for [`bevy_renet`](https://docs.rs/bevy_renet).

# Getting started

This guide assumes that you have already read the [quick start guide](https://docs.rs/bevy_replicon#quick-start)
for `bevy_replicon`.

## Modules

Renet API is re-exported from this drate under [`renet`] module. Features from `bevy_renet` are exposed via
`renet_*` features, which also re-export the corresponding transport modules. Like in `bevy_renet`, the netcode
transport is enabled by default.

So you don't need to include `bevy_renet` or `renet` in your `Cargo.toml`.

## Plugins

Add [`RepliconRenetPlugins`] along with [`RepliconPlugins`]:

```
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;

let mut app = App::new();
app.add_plugins((MinimalPlugins, RepliconPlugins, RepliconRenetPlugins));
```

Similar to Replicon, we provide `client` and `server` features. These automatically enable the corresponding
features in `bevy_replicon`.

The plugins in [`RepliconRenetPlugins`] automatically include the `renet` plugins, so you don't need to add
them manually. If the `renet_transport` feature is enabled, the netcode plugins will also be added automatically.

## Server and client creation

Just like with regular `bevy_renet`, you need to create the
[`RenetClient`](renet::RenetClient) and [`NetcodeClientTransport`](bevy_renet::netcode::NetcodeClientTransport) **or**
[`RenetServer`](renet::RenetServer) and [`NetcodeServerTransport`](bevy_renet::netcode::NetcodeServerTransport)
resources from Renet.

For steam transport you need to activate the corresponding feature and use its transport resource instead.

This crate will automatically manage their integration with Replion.

<div class="warning">

Never insert client and server resources in the same app, it will cause a replication loop.
See the Replicon's quick start guide for more details.

</div>

The only Replicon-specific part is channels. You need to get them from the [`RepliconChannels`] resource.
This crate provides the [`RenetChannelsExt`] extension trait to conveniently create renet channels from it:

```
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{renet::ConnectionConfig, RenetChannelsExt};

fn init(channels: Res<RepliconChannels>) {
    let connection_config = ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    };

    // Use this config for `RenetServer` or `RenetClient`
}
```

For a full example of how to initialize a server or client see examples in the repository.

<div class="warning">

Channels need to be obtained only **after** registering all replication components and remote events.

</div>

## Replicon conditions

The crate updates the running state of [`RepliconServer`] and connection state of [`RepliconClient`]
based on the states of [`RenetServer`](renet::RenetServer) and [`RenetClient`](renet::RenetServer)
in [`PreUpdate`].

This means that [Replicon conditions](bevy_replicon::shared::common_conditions) won't work in schedules
like [`Startup`]. As a workaround, you can directly check if renet's resources are present. This may be resolved
in the future once we have [observers for resources](https://github.com/bevyengine/bevy/issues/12231)
to immediately react to changes.
*/
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

use std::time::Duration;

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

/// Plugin group for all Replicon renet backend plugins.
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
    ///
    /// - [`SendType::ReliableUnordered::resend_time`] and [`SendType::ReliableOrdered::resend_time`] will be
    ///   set to 300 ms.
    /// - [`ChannelConfig::max_memory_usage_bytes`] will be set to `5 * 1024 * 1024`.
    ///
    /// You can configure these parameters after creation. However, do not change [`SendType`], as Replicon relies
    /// on its defined delivery guarantees.
    ///
    /// # Examples
    ///
    /// Configure event channels using
    /// [`RemoteEventRegistry`](bevy_replicon::shared::event::remote_event_registry::RemoteEventRegistry):
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_replicon::{prelude::*, shared::event::remote_event_registry::RemoteEventRegistry};
    /// # use bevy_replicon_renet::RenetChannelsExt;
    /// # let channels = RepliconChannels::default();
    /// # let registry = RemoteEventRegistry::default();
    /// fn init(channels: Res<RepliconChannels>, event_registry: Res<RemoteEventRegistry>) {
    ///     let mut server_configs = channels.server_configs();
    ///     let fire_id = event_registry.server_channel::<Fire>().unwrap();
    ///     let fire_channel = &mut server_configs[fire_id];
    ///     fire_channel.max_memory_usage_bytes = 2048;
    ///     // Use `server_configs` to create `RenetServer`.
    /// }
    ///
    /// #[derive(Event)]
    /// struct Fire;
    /// ```
    ///
    /// Configure replication channels:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_replicon::{prelude::*, shared::backend::replicon_channels::ServerChannel};
    /// # use bevy_replicon_renet::RenetChannelsExt;
    /// # let channels = RepliconChannels::default();
    /// let mut server_configs = channels.server_configs();
    /// let channel = &mut server_configs[ServerChannel::Updates as usize];
    /// channel.max_memory_usage_bytes = 4090;
    /// ```
    fn server_configs(&self) -> Vec<ChannelConfig>;

    /// Same as [`RenetChannelsExt::server_configs`], but for clients.
    fn client_configs(&self) -> Vec<ChannelConfig>;
}

impl RenetChannelsExt for RepliconChannels {
    fn server_configs(&self) -> Vec<ChannelConfig> {
        let channels = self.server_channels();
        assert!(
            channels.len() <= u8::MAX as usize,
            "number of server channels shouldn't exceed `u8::MAX`"
        );

        create_configs(channels)
    }

    fn client_configs(&self) -> Vec<ChannelConfig> {
        let channels = self.client_channels();
        assert!(
            channels.len() <= u8::MAX as usize,
            "number of client channels shouldn't exceed `u8::MAX`"
        );

        create_configs(channels)
    }
}

/// Converts Replicon channels into renet channel configs.
fn create_configs(channels: &[Channel]) -> Vec<ChannelConfig> {
    let mut channel_configs = Vec::with_capacity(channels.len());
    for (index, &channel) in channels.iter().enumerate() {
        let send_type = match channel {
            Channel::Unreliable => SendType::Unreliable,
            Channel::Unordered => SendType::ReliableUnordered {
                resend_time: Duration::from_millis(300),
            },
            Channel::Ordered => SendType::ReliableOrdered {
                resend_time: Duration::from_millis(300),
            },
        };
        let config = ChannelConfig {
            channel_id: index as u8,
            max_memory_usage_bytes: 5 * 1024 * 1024,
            send_type,
        };

        debug!("creating channel config `{config:?}`");
        channel_configs.push(config);
    }
    channel_configs
}
