use crate::{
  StateSourcedEffectorMessageAdapter, StateSourcedEffectorSignal,
  state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregateCommand {
  DomainCommand(u32),
  Signal(StateSourcedEffectorSignal<u32>),
}

#[test]
fn wraps_state_sourced_signal_into_private_message() {
  let adapter = message_adapter();
  let signal = StateSourcedEffectorSignal::StatePersisted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    state:    42,
    revision: 7,
  };

  let message = adapter.wrap_signal(signal);

  assert!(
    matches!(
      message,
      AggregateCommand::Signal(StateSourcedEffectorSignal::StatePersisted { auth: _, state: 42, revision: 7 })
    ),
    "adapter must wrap public state-sourced signal into aggregate-private message",
  );
}

#[test]
fn unwraps_state_sourced_signal_from_private_message() {
  let adapter = message_adapter();
  let message = AggregateCommand::Signal(StateSourcedEffectorSignal::StateDeleted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    revision: 9,
  });

  let signal = adapter.unwrap_signal(&message);

  assert!(
    matches!(signal, Some(StateSourcedEffectorSignal::StateDeleted { auth: _, revision: 9 })),
    "adapter must borrow state-sourced signal from aggregate-private message",
  );
}

#[test]
fn unwrap_returns_none_for_domain_command() {
  let adapter = message_adapter();
  let message = AggregateCommand::DomainCommand(1);

  assert!(adapter.unwrap_signal(&message).is_none());
}

fn message_adapter() -> StateSourcedEffectorMessageAdapter<u32, AggregateCommand> {
  StateSourcedEffectorMessageAdapter::new(AggregateCommand::Signal, |message| match message {
    | AggregateCommand::Signal(signal) => Some(signal),
    | AggregateCommand::DomainCommand(_) => None,
  })
}
