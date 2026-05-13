use core::time::Duration;
use std::{string::String, thread, time::Instant};

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::actor::{
  extension::ExtensionInstallers, scheduler::SchedulerConfig, setup::ActorSystemConfig,
};
use fraktor_actor_core_typed_rs::{Behavior, TypedActorRef, TypedActorSystem, dsl::Behaviors};
use fraktor_persistence_core_kernel_rs::{
  extension::PersistenceExtensionInstaller, journal::InMemoryJournal, snapshot::InMemorySnapshotStore,
};
use fraktor_persistence_core_typed_rs::{
  PersistenceEffector, PersistenceEffectorConfig, PersistenceEffectorMessageAdapter, PersistenceEffectorSignal,
  PersistenceId, PersistenceMode, RetentionCriteria, SnapshotCriteria,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum AccountState {
  NotCreated,
  Created(BankAccount),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BankAccount {
  balance: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AccountEvent {
  Opened { initial_balance: i64 },
  Deposited { amount: i64 },
}

#[derive(Clone, Debug)]
enum AccountCommand {
  Open { initial_balance: i64, reply_to: TypedActorRef<String> },
  Deposit { amount: i64, reply_to: TypedActorRef<String> },
  GetBalance { reply_to: TypedActorRef<i64> },
  Persistence(PersistenceEffectorSignal<AccountState, AccountEvent>),
}

impl BankAccount {
  fn open(initial_balance: i64) -> Result<(Self, AccountEvent), &'static str> {
    if initial_balance < 0 {
      return Err("initial balance must be non-negative");
    }
    Ok((Self { balance: initial_balance }, AccountEvent::Opened { initial_balance }))
  }

  fn deposit(&self, amount: i64) -> Result<(Self, AccountEvent), &'static str> {
    if amount <= 0 {
      return Err("deposit amount must be positive");
    }
    let next = Self { balance: self.balance + amount };
    Ok((next, AccountEvent::Deposited { amount }))
  }
}

fn apply_event(state: &AccountState, event: &AccountEvent) -> AccountState {
  match (state, event) {
    | (_, AccountEvent::Opened { initial_balance }) => AccountState::Created(BankAccount { balance: *initial_balance }),
    | (AccountState::Created(account), AccountEvent::Deposited { amount }) => {
      AccountState::Created(BankAccount { balance: account.balance + amount })
    },
    | (AccountState::NotCreated, AccountEvent::Deposited { .. }) => AccountState::NotCreated,
  }
}

fn account_config() -> PersistenceEffectorConfig<AccountState, AccountEvent, AccountCommand> {
  let message_adapter = PersistenceEffectorMessageAdapter::new(AccountCommand::Persistence, |message| match message {
    | AccountCommand::Persistence(signal) => Some(signal),
    | _ => None,
  });
  PersistenceEffectorConfig::new(
    PersistenceId::of_unique_id("typed-bank-account-1"),
    AccountState::NotCreated,
    apply_event,
  )
  .with_persistence_mode(PersistenceMode::Persisted)
  .with_snapshot_criteria(SnapshotCriteria::every(2))
  .with_retention_criteria(RetentionCriteria::snapshot_every(2, 2))
  .with_message_adapter(message_adapter)
}

fn account_behavior(
  state: AccountState,
  effector: PersistenceEffector<AccountState, AccountEvent, AccountCommand>,
) -> Behavior<AccountCommand> {
  match state {
    | AccountState::NotCreated => not_created(effector),
    | AccountState::Created(account) => created(account, effector),
  }
}

fn not_created(effector: PersistenceEffector<AccountState, AccountEvent, AccountCommand>) -> Behavior<AccountCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | AccountCommand::Open { initial_balance, reply_to } => match BankAccount::open(*initial_balance) {
      | Ok((account, event)) => {
        let next_state = AccountState::Created(account.clone());
        let next_effector = effector.clone();
        let mut reply_to = reply_to.clone();
        effector.persist_event_with_snapshot(ctx, event, next_state, true, move |_event| {
          reply_to.tell(String::from("opened"));
          Ok(created(account, next_effector))
        })
      },
      | Err(reason) => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(String::from(reason));
        Ok(Behaviors::same())
      },
    },
    | AccountCommand::Deposit { reply_to, .. } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(String::from("account is not created"));
      Ok(Behaviors::same())
    },
    | AccountCommand::GetBalance { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(0);
      Ok(Behaviors::same())
    },
    | AccountCommand::Persistence(_) => Ok(Behaviors::unhandled()),
  })
}

fn created(
  account: BankAccount,
  effector: PersistenceEffector<AccountState, AccountEvent, AccountCommand>,
) -> Behavior<AccountCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | AccountCommand::Open { reply_to, .. } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(String::from("account already exists"));
      Ok(Behaviors::same())
    },
    | AccountCommand::Deposit { amount, reply_to } => match account.deposit(*amount) {
      | Ok((next_account, event)) => {
        let next_state = AccountState::Created(next_account.clone());
        let next_effector = effector.clone();
        let mut reply_to = reply_to.clone();
        effector.persist_event_with_snapshot(ctx, event, next_state, false, move |_event| {
          reply_to.tell(String::from("deposited"));
          Ok(created(next_account, next_effector))
        })
      },
      | Err(reason) => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(String::from(reason));
        Ok(Behaviors::same())
      },
    },
    | AccountCommand::GetBalance { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(account.balance);
      Ok(Behaviors::same())
    },
    | AccountCommand::Persistence(_) => Ok(Behaviors::unhandled()),
  })
}

fn system_config() -> ActorSystemConfig {
  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  ActorSystemConfig::new(StdTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers)
}

fn main() {
  let props = PersistenceEffector::props(account_config(), |state, effector| Ok(account_behavior(state, effector)));
  let system = TypedActorSystem::<AccountCommand>::create_from_props(&props, system_config()).expect("system");
  let termination = system.when_terminated();
  let mut account = system.user_guardian_ref();

  let opened = ask_text(&mut account, |reply_to| AccountCommand::Open { initial_balance: 100, reply_to });
  let deposited = ask_text(&mut account, |reply_to| AccountCommand::Deposit { amount: 25, reply_to });
  let balance = ask_balance(&mut account);

  println!("typed_persistence_effector results: opened={opened}, deposited={deposited}, balance={balance}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn ask_text(
  account: &mut TypedActorRef<AccountCommand>,
  request: impl FnOnce(TypedActorRef<String>) -> AccountCommand,
) -> String {
  let response = account.ask::<String, _>(request);
  let mut future = response.future().clone();
  wait_until(|| future.is_ready(), Duration::from_secs(10));
  future.try_take().expect("text reply").expect("text payload")
}

fn ask_balance(account: &mut TypedActorRef<AccountCommand>) -> i64 {
  let response = account.ask::<i64, _>(|reply_to| AccountCommand::GetBalance { reply_to });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready(), Duration::from_secs(10));
  future.try_take().expect("balance reply").expect("balance payload")
}

fn wait_until(mut condition: impl FnMut() -> bool, timeout: Duration) {
  let started = Instant::now();
  while started.elapsed() < timeout {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
