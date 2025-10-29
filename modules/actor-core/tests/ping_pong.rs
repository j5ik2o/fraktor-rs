#![allow(dead_code)]

extern crate alloc;

use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};
use std::sync::Mutex;

use actor_core::{Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyOwnedMessage, ChildRef, Props};

static MESSAGE_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Start;

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<Start>().is_some() {
      let recorder = ctx.spawn_child(&Props::new(recorder_factory))?;
      let ping = ctx.spawn_child(&Props::new(ping_factory))?;

      let start_ping = StartPing { target: recorder.clone(), count: 3 };
      if ping.tell(AnyOwnedMessage::new(start_ping)).is_err() {
        return Err(ActorError::recoverable("send_failed"));
      }
    }
    Ok(())
  }
}

struct RecorderActor;

impl Actor for RecorderActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(text) = msg.downcast_ref::<String>() {
      if let Ok(mut guard) = MESSAGE_LOG.lock() {
        guard.push(text.clone());
      }
    }
    Ok(())
  }
}

struct PingActor;

struct StartPing {
  target: ChildRef,
  count:  u32,
}

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = msg.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let payload = format_message(index);
        if cmd.target.tell(AnyOwnedMessage::new(payload.clone())).is_err() {
          return Err(ActorError::recoverable("send_failed"));
        }
      }
    }
    Ok(())
  }
}

fn format_message(index: u32) -> String {
  format!("ping-{}", index + 1)
}

fn guardian_factory() -> Box<dyn Actor> {
  Box::new(TestGuardian)
}

fn recorder_factory() -> Box<dyn Actor> {
  Box::new(RecorderActor)
}

fn ping_factory() -> Box<dyn Actor> {
  Box::new(PingActor)
}

#[test]
fn ping_messages_are_delivered() {
  MESSAGE_LOG.lock().unwrap().clear();

  let system = ActorSystem::new(Props::new(guardian_factory)).expect("system");
  system.user_guardian_ref().tell(AnyOwnedMessage::new(Start)).expect("start");

  let log = MESSAGE_LOG.lock().unwrap();
  assert_eq!(log.as_slice(), &["ping-1".to_string(), "ping-2".to_string(), "ping-3".to_string()]);
}
