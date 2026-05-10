use crate::{StreamError, r#impl::materialization::StreamIslandCommand};

#[test]
fn stream_island_command_is_cloneable_for_actor_mailbox() {
  fn assert_clone<T: Clone>() {}

  assert_clone::<StreamIslandCommand>();
}

#[test]
fn drive_command_can_be_constructed() {
  let _command = StreamIslandCommand::Drive;
}

#[test]
fn cancel_command_can_omit_cause() {
  let command = StreamIslandCommand::Cancel { cause: None };

  match command {
    | StreamIslandCommand::Cancel { cause } => assert_eq!(cause, None),
    | _ => panic!("expected cancel command"),
  }
}

#[test]
fn cancel_command_preserves_cause() {
  let cause = StreamError::Failed;
  let command = StreamIslandCommand::Cancel { cause: Some(cause.clone()) };

  match command {
    | StreamIslandCommand::Cancel { cause: Some(actual) } => assert_eq!(actual, cause),
    | _ => panic!("expected cancel command with cause"),
  }
}

#[test]
fn shutdown_command_can_be_constructed() {
  let _command = StreamIslandCommand::Shutdown;
}

#[test]
fn abort_command_preserves_error() {
  let error = StreamError::Failed;
  let command = StreamIslandCommand::Abort(error.clone());

  match command {
    | StreamIslandCommand::Abort(actual) => assert_eq!(actual, error),
    | _ => panic!("expected abort command"),
  }
}
