use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_remote_adaptor_std_rs::std::{
  extension_installer::RemotingExtensionInstaller, transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{address::Address, config::RemoteConfig};
use fraktor_utils_core_rs::core::sync::ArcShared;

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let props = Props::from_fn(|| NoopActor);
  let advertised_address = Address::new("remote-showcase", "127.0.0.1", 0);
  let transport = TcpRemoteTransport::new("127.0.0.1:0", vec![advertised_address]);
  let remote_config = RemoteConfig::new("127.0.0.1");
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, remote_config));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer);
  let config = ActorSystemConfig::new(StdTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  println!("remote_lifecycle initialized remoting extension for 127.0.0.1");

  system.terminate().expect("terminate");
}
