#![allow(dead_code)]

extern crate alloc;

use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};
use std::sync::Mutex;

use actor_core::{
  Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyOwnedMessage, ChildRef, Props, SendError,
};

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PRE_START_COUNT: Mutex<u32> = Mutex::new(0);
static ERROR_LOG: Mutex<Vec<SendError>> = Mutex::new(Vec::new());

struct InitRecoverable;
struct RunRecoverable;
struct InitFatal;
struct RunFatal;
struct CauseRecoverable;
struct CauseFatal;
struct Record(String);

struct GuardianRecoverable {
  child: Option<ChildRef>,
}

impl Actor for GuardianRecoverable {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<InitRecoverable>().is_some() {
      let child = ctx.spawn_child(&Props::new(recoverable_child_factory))?;
      self.child = Some(child);
    } else if msg.downcast_ref::<RunRecoverable>().is_some() {
      let Some(child) = self.child.clone() else {
        return Ok(());
      };
      child.tell(AnyOwnedMessage::new(CauseRecoverable)).expect("recoverable send");
      child.tell(AnyOwnedMessage::new(Record("ok".to_string()))).expect("post restart send");
    }
    Ok(())
  }
}

struct GuardianFatal {
  child: Option<ChildRef>,
}

impl Actor for GuardianFatal {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<InitFatal>().is_some() {
      let child = ctx.spawn_child(&Props::new(fatal_child_factory))?;
      self.child = Some(child);
    } else if msg.downcast_ref::<RunFatal>().is_some() {
      let Some(child) = self.child.clone() else {
        return Ok(());
      };
      if let Err(err) = child.tell(AnyOwnedMessage::new(CauseFatal)) {
        ERROR_LOG.lock().unwrap().push(err.clone());
      }
      if let Err(err) = child.tell(AnyOwnedMessage::new(Record("ignored".to_string()))) {
        ERROR_LOG.lock().unwrap().push(err);
      }
    }
    Ok(())
  }
}

struct RecoverableChild;

impl Actor for RecoverableChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    *PRE_START_COUNT.lock().unwrap() += 1;
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<CauseRecoverable>().is_some() {
      return Err(ActorError::recoverable("recoverable"));
    }
    if let Some(record) = msg.downcast_ref::<Record>() {
      LOG.lock().unwrap().push(record.0.clone());
    }
    Ok(())
  }
}

struct FatalChild;

impl Actor for FatalChild {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<CauseFatal>().is_some() {
      return Err(ActorError::fatal("fatal"));
    }
    Ok(())
  }
}

fn guardian_recoverable_factory() -> Box<dyn Actor> {
  Box::new(GuardianRecoverable { child: None })
}

fn guardian_fatal_factory() -> Box<dyn Actor> {
  Box::new(GuardianFatal { child: None })
}

fn recoverable_child_factory() -> Box<dyn Actor> {
  Box::new(RecoverableChild)
}

fn fatal_child_factory() -> Box<dyn Actor> {
  Box::new(FatalChild)
}

#[test]
fn recoverable_error_restarts_child() {
  LOG.lock().unwrap().clear();
  *PRE_START_COUNT.lock().unwrap() = 0;

  let system = ActorSystem::new(Props::new(guardian_recoverable_factory)).expect("system");
  system.user_guardian_ref().tell(AnyOwnedMessage::new(InitRecoverable)).expect("init");
  assert_eq!(*PRE_START_COUNT.lock().unwrap(), 1);

  system.user_guardian_ref().tell(AnyOwnedMessage::new(RunRecoverable)).expect("run");

  assert_eq!(*PRE_START_COUNT.lock().unwrap(), 2);
  assert_eq!(LOG.lock().unwrap().as_slice(), &["ok".to_string()]);
}

#[test]
fn fatal_error_stops_child() {
  ERROR_LOG.lock().unwrap().clear();

  let system = ActorSystem::new(Props::new(guardian_fatal_factory)).expect("system");
  system.user_guardian_ref().tell(AnyOwnedMessage::new(InitFatal)).expect("init");

  system.user_guardian_ref().tell(AnyOwnedMessage::new(RunFatal)).expect("run");

  let errors = ERROR_LOG.lock().unwrap();
  assert!(matches!(errors.get(0), Some(SendError::ActorFailure(ActorError::Fatal("fatal")))));
  assert!(matches!(errors.get(1), Some(SendError::UnknownPid)));
}
