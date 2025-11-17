#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::{format, string::String};
use core::time::Duration;

use fraktor_actor_core_rs::core::{
  actor_prim::{Actor, ActorContext, actor_path::ActorPathParser, actor_ref::ActorRef},
  config::{ActorSystemConfig, RemotingConfig},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};

struct Start;

struct DumpPath {
  reply_to: ActorRef,
}

struct PathReport {
  canonical: String,
}

struct InspectorGuardian;

impl Actor for InspectorGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      if let Some(path) = ctx.self_ref().path() {
        log_path("user guardian", &path.to_canonical_uri());
      }

      let child = ctx
        .spawn_child(&Props::from_fn(|| WorkerActor))
        .map_err(|error| ActorError::recoverable(format!("worker spawn failed: {error:?}")))?;

      if let Some(child_path) = child.actor_ref().path() {
        log_path("child (guardian view)", &child_path.to_canonical_uri());
      }

      let request = DumpPath { reply_to: ctx.self_ref() };
      child.actor_ref().tell(AnyMessage::new(request)).map_err(|error| ActorError::from_send_error(&error))?;
    } else if let Some(report) = message.downcast_ref::<PathReport>() {
      log_path("child (self view)", &report.canonical);
      let parsed = ActorPathParser::parse(&report.canonical)
        .map_err(|_| ActorError::recoverable("failed to parse canonical path"))?;
      log_relative(&parsed.to_relative_string());
      ctx.system().terminate().map_err(|error| ActorError::from_send_error(&error))?;
    }
    Ok(())
  }
}

struct WorkerActor;

impl Actor for WorkerActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(request) = message.downcast_ref::<DumpPath>() {
      if let Some(path) = ctx.self_ref().path() {
        let canonical = path.to_canonical_uri();
        let report = PathReport { canonical };
        request.reply_to.tell(AnyMessage::new(report)).map_err(|error| ActorError::from_send_error(&error))?;
      }
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn log_path(label: &str, value: &str) {
  println!("[actor_path] {label}: {value}");
}

#[cfg(target_os = "none")]
fn log_path(_: &str, _: &str) {}

#[cfg(not(target_os = "none"))]
fn log_relative(value: &str) {
  println!("[actor_path] relative path: {value}");
}

#[cfg(target_os = "none")]
fn log_relative(_: &str) {}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let remoting = RemotingConfig::default()
    .with_canonical_host("dev.local")
    .with_canonical_port(25252)
    .with_quarantine_duration(Duration::from_secs(300));
  let config = ActorSystemConfig::default().with_system_name("fraktor-inspector").with_remoting(remoting);

  let props = Props::from_fn(|| InspectorGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("actor system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start inspector");

  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
