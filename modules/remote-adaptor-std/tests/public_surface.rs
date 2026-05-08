//! Public surface checks for the std remote adaptor.

use fraktor_remote_adaptor_std_rs::std::{
  extension_installer::RemotingExtensionInstaller, provider::StdRemoteActorRefProviderInstaller,
  transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  config::RemoteConfig,
};
use fraktor_utils_core_rs::core::sync::ArcShared;

const STD_RS: &str = include_str!("../src/std.rs");
const PROVIDER_RS: &str = include_str!("../src/std/provider.rs");
const PROVIDER_DISPATCH_RS: &str = include_str!("../src/std/provider/dispatch.rs");
const TCP_RS: &str = include_str!("../src/std/transport/tcp.rs");
const TCP_BASE_RS: &str = include_str!("../src/std/transport/tcp/base.rs");

#[test]
fn user_facing_adapter_boundary_imports_compile() {
  let address = Address::new("surface", "127.0.0.1", 0);
  let transport = TcpRemoteTransport::new("127.0.0.1:0", vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, RemoteConfig::new("127.0.0.1")));
  let _provider_installer =
    StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(UniqueAddress::new(address, 1), installer);
}

#[test]
fn runtime_internal_modules_are_not_publicly_exported() {
  assert!(!STD_RS.contains("pub mod association;"));
  assert!(!STD_RS.contains("pub mod watcher_actor;"));
  assert!(!STD_RS.contains("pub use tokio_remote_event_receiver"));
  assert!(!TCP_RS.contains("pub use client::TcpClient;"));
  assert!(!TCP_RS.contains("pub use server::TcpServer;"));
  assert!(!TCP_RS.contains("pub use frame_codec::WireFrameCodec;"));
  assert!(!TCP_RS.contains("pub use frame_codec_error::FrameCodecError;"));
  assert!(!TCP_RS.contains("pub use inbound_frame_event::InboundFrameEvent;"));
  assert!(!PROVIDER_RS.contains("pub use path_remote_actor_ref_provider::PathRemoteActorRefProvider;"));
  assert!(!PROVIDER_RS.contains("pub use remote_actor_ref_sender::RemoteActorRefSender;"));
  assert!(!PROVIDER_DISPATCH_RS.contains("pub fn new("));
}

#[test]
fn tcp_remote_transport_public_methods_do_not_expose_internal_types() {
  assert!(!TCP_BASE_RS.contains("pub fn take_inbound_receiver"));
  assert!(!TCP_BASE_RS.contains("pub fn clients"));
  assert!(!TCP_BASE_RS.contains("pub fn connect_peer_async"));
  assert!(!TCP_BASE_RS.contains("pub fn connect_peer_blocking"));
  assert!(!TCP_BASE_RS.contains("pub fn send_handshake"));
  assert!(!TCP_BASE_RS.contains("pub fn send_control"));
}
