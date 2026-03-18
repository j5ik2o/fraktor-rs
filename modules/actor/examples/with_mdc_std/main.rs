//! Demonstrates `Behaviors::with_mdc()` for MDC (Mapped Diagnostic Context) support.
//!
//! Static MDC entries are applied to every message. Per-message MDC entries
//! are computed dynamically. Both sets of entries are emitted as tracing span
//! fields. This mirrors Pekko's `Behaviors.withMdc`.

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use alloc::collections::BTreeMap;

extern crate alloc;

use std::{thread, time::Duration};

use fraktor_actor_rs::{
  core::typed::{Behavior, Behaviors as CoreBehaviors, TypedActorSystem, TypedProps},
  std::typed::Behaviors,
};
use fraktor_utils_rs::core::sync::SharedAccess;

#[derive(Clone, Debug)]
enum AppMsg {
  Request { id: u32 },
  Shutdown,
}

fn app_behavior() -> Behavior<AppMsg> {
  let mut static_mdc = BTreeMap::new();
  static_mdc.insert("service".into(), "my-app".into());
  static_mdc.insert("version".into(), "1.0".into());

  let inner = Behaviors::receive_message(|_ctx, msg: &AppMsg| {
    match msg {
      | AppMsg::Request { id } => {
        tracing::info!(request_id = id, "processing request");
      },
      | AppMsg::Shutdown => {
        tracing::info!("shutting down");
      },
    }
    Ok(CoreBehaviors::same())
  });

  Behaviors::with_mdc(
    static_mdc,
    |msg: &AppMsg| {
      let mut mdc = BTreeMap::new();
      if let AppMsg::Request { id } = msg {
        mdc.insert("request_id".into(), alloc::format!("{id}"));
      }
      mdc
    },
    inner,
  )
}

fn main() {
  let subscriber = tracing_subscriber::FmtSubscriber::builder().with_max_level(tracing::Level::DEBUG).finish();
  tracing::subscriber::set_global_default(subscriber).expect("subscriber");

  let props = TypedProps::from_behavior_factory(app_behavior);
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");

  system.user_guardian_ref().tell(AppMsg::Request { id: 1 }).expect("request 1");
  system.user_guardian_ref().tell(AppMsg::Request { id: 2 }).expect("request 2");
  system.user_guardian_ref().tell(AppMsg::Shutdown).expect("shutdown");

  thread::sleep(Duration::from_millis(100));

  system.terminate().expect("terminate");
  let termination = system.when_terminated();
  while !termination.with_read(|inner| inner.is_ready()) {
    thread::sleep(Duration::from_millis(10));
  }

  tracing::info!("MDC example completed.");
}
