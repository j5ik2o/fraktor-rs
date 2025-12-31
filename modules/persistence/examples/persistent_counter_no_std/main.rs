//! Basic untyped persistence walkthrough.

extern crate alloc;

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::{
  actor::actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_persistence_rs::core::{
  AtLeastOnceDelivery, AtLeastOnceDeliveryConfig, Eventsourced, InMemoryJournal, InMemorySnapshotStore, Journal,
  JournalActor, JournalMessage, JournalResponse, PersistentActor, PersistentActorBase, PersistentRepr, Snapshot,
  SnapshotActor, SnapshotMessage, SnapshotMetadata, SnapshotResponse, SnapshotStore,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender<TB> for TestSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender =
    ActorRefGeneric::new(fraktor_actor_rs::core::actor::Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

#[derive(Clone)]
enum Event {
  Incremented(i32),
}

struct CounterActor {
  value: i32,
  base:  PersistentActorBase<CounterActor, TB>,
}

impl CounterActor {
  fn new(persistence_id: &str, journal: ActorRefGeneric<TB>, snapshot: ActorRefGeneric<TB>) -> Self {
    Self { value: 0, base: PersistentActorBase::new(persistence_id.into(), journal, snapshot) }
  }
}

impl Eventsourced<TB> for CounterActor {
  fn persistence_id(&self) -> &str {
    self.base.persistence_id()
  }

  fn journal_actor_ref(&self) -> &ActorRefGeneric<TB> {
    self.base.journal_actor_ref()
  }

  fn snapshot_actor_ref(&self) -> &ActorRefGeneric<TB> {
    self.base.snapshot_actor_ref()
  }

  fn receive_recover(&mut self, event: &PersistentRepr) {
    if let Some(event) = event.downcast_ref::<Event>() {
      let Event::Incremented(delta) = event;
      self.value += delta;
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i32>() {
      self.value = *value;
    }
  }

  fn receive_command(
    &mut self,
    _ctx: &mut fraktor_actor_rs::core::actor::ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.base.last_sequence_nr()
  }
}

impl PersistentActor<TB> for CounterActor {
  fn base(&self) -> &PersistentActorBase<Self, TB> {
    &self.base
  }

  fn base_mut(&mut self) -> &mut PersistentActorBase<Self, TB> {
    &mut self.base
  }
}

struct NoopActor;

impl fraktor_actor_rs::core::actor::Actor<TB> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut fraktor_actor_rs::core::actor::ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn main() {
  // 日本語コメント: InMemory 実装は core::future::ready() を返す
  let mut journal = InMemoryJournal::new();
  let _ready = journal.write_messages(&[]);
  let snapshot_backend = InMemorySnapshotStore::new();
  drop(snapshot_backend.load_snapshot("counter-1", fraktor_persistence_rs::core::SnapshotSelectionCriteria::latest()));

  // 日本語コメント: JournalActor/SnapshotActor を生成する（メッセージパッシングで使用）
  let _journal_actor = JournalActor::<InMemoryJournal, TB>::new(InMemoryJournal::new());
  let _snapshot_actor = SnapshotActor::<InMemorySnapshotStore, TB>::new(InMemorySnapshotStore::new());

  // 日本語コメント: テスト用の送信先を準備する
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut actor = CounterActor::new("counter-1", journal_ref, snapshot_ref);

  // 日本語コメント: 永続化イベントをキューに積み、WriteMessages を送る
  let system = fraktor_actor_rs::core::system::ActorSystemGeneric::from_state(
    fraktor_actor_rs::core::system::SystemStateSharedGeneric::new(
      fraktor_actor_rs::core::system::SystemStateGeneric::new(),
    ),
  );
  let pid = fraktor_actor_rs::core::actor::Pid::new(1, 1);
  let props = fraktor_actor_rs::core::props::PropsGeneric::from_fn(|| NoopActor);
  let cell =
    fraktor_actor_rs::core::actor::ActorCellGeneric::create(system.state(), pid, None, String::from("self"), &props)
      .expect("create actor cell");
  system.state().register_cell(cell);
  let mut ctx = fraktor_actor_rs::core::actor::ActorContextGeneric::new(&system, pid);
  actor.persist(&mut ctx, Event::Incremented(1), |actor, event| {
    let Event::Incremented(delta) = event;
    actor.value += delta;
  });
  actor.base.flush_batch(ActorRefGeneric::null());

  // 日本語コメント: JournalActor からの成功レスポンスを模擬する
  if let Some(message) = journal_store.lock().first()
    && let Some(JournalMessage::WriteMessages { messages, .. }) = message.payload().downcast_ref::<JournalMessage<TB>>()
    && let Some(repr) = messages.first()
  {
    let response = JournalResponse::WriteMessageSuccess { repr: repr.clone(), instance_id: 1 };
    actor.handle_journal_response(&response);
  }

  // 日本語コメント: スナップショット保存メッセージを送る
  let metadata = SnapshotMetadata::new("counter-1", actor.base.current_sequence_nr(), 0);
  let snapshot_payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(actor.value);
  let snapshot_message: SnapshotMessage<TB> =
    SnapshotMessage::SaveSnapshot { metadata, snapshot: snapshot_payload, sender: ActorRefGeneric::null() };
  snapshot_store.lock().push(AnyMessageGeneric::new(snapshot_message));

  // 日本語コメント: スナップショット復元の流れを模擬する
  let snapshot =
    Snapshot::new(SnapshotMetadata::new("counter-1", actor.base.current_sequence_nr(), 0), ArcShared::new(actor.value));
  let response = SnapshotResponse::LoadSnapshotResult {
    snapshot:       Some(snapshot),
    to_sequence_nr: actor.base.current_sequence_nr(),
  };
  actor.handle_snapshot_response(&response, &mut ctx);

  // 日本語コメント: AtLeastOnceDelivery の基本利用
  let mut delivery: AtLeastOnceDelivery<TB> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());
  let delivery_id = delivery.next_delivery_id();
  let _ = delivery_id;
}
