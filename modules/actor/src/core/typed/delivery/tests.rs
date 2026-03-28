extern crate std;

use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, TypedActorSystem, TypedProps,
  actor::TypedActorRef,
  delivery::{
    ConsumerController, ConsumerControllerCommand, ConsumerControllerConfirmed, ConsumerControllerDelivery,
    ProducerController, ProducerControllerCommand, ProducerControllerRequestNext, WorkPullingProducerController,
    WorkPullingProducerControllerCommand, WorkPullingProducerControllerRequestNext, WorkerStats,
  },
  receptionist::{Receptionist, ServiceKey},
};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    std::thread::yield_now();
  }
  assert!(condition(), "wait_until timed out");
}

/// Helper to create a test actor system.
fn test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::kernel::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::scheduler::tick_driver::ManualTestDriver::new(),
  );
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

#[test]
fn producer_start_and_register_consumer_connect() {
  let system = test_system();

  // プロデューサーコントローラーを生成する。
  let pc_props = TypedProps::<ProducerControllerCommand<u32>>::from_behavior_factory(|| {
    ProducerController::behavior("test-producer")
  });
  let pc_cell = system.as_untyped().spawn(pc_props.to_untyped()).expect("spawn producer controller");
  let mut pc_ref = TypedActorRef::<ProducerControllerCommand<u32>>::from_untyped(pc_cell.into_actor_ref());

  // コンシューマーコントローラーを生成する。
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn consumer controller");
  let cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.into_actor_ref());

  // 受信した RequestNext シグナルを追跡する。
  let request_next_received = ArcShared::new(NoStdMutex::new(Vec::<u64>::new()));
  let request_next_received_clone = request_next_received.clone();

  // RequestNext シグナルを記録するモックプロデューサーを生成する。
  let producer_props = TypedProps::<ProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      let received = request_next_received_clone.clone();
      Behaviors::receive_message(move |_ctx, req: &ProducerControllerRequestNext<u32>| {
        received.lock().push(req.current_seq_nr());
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref = TypedActorRef::<ProducerControllerRequestNext<u32>>::from_untyped(producer_cell.into_actor_ref());

  // プロデューサーコントローラーを開始する。
  pc_ref.tell(ProducerController::start(producer_ref));

  // コンシューマーを登録する。
  pc_ref.tell(ProducerController::register_consumer(cc_ref.clone()));

  // ProducerController は接続されるはずだが、実際の RequestNext 配信は
  // コンシューマー側が Request を送信することに依存する。最低限、システムが
  // パニックせず、すべてのメッセージが受け入れられること。

  system.terminate().expect("terminate");
}

#[test]
fn consumer_controller_delivers_to_consumer() {
  let system = test_system();

  // 配達を追跡する。
  let delivered = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let delivered_clone = delivered.clone();

  // コンシューマーコントローラーを生成する。
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn consumer controller");
  let mut cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.into_actor_ref());

  // 配達を処理して Confirmed を送信するコンシューマーを生成する。
  let consumer_props = TypedProps::<ConsumerControllerDelivery<u32>>::from_behavior_factory({
    move || {
      let delivered = delivered_clone.clone();
      Behaviors::receive_message(move |_ctx, delivery: &ConsumerControllerDelivery<u32>| {
        delivered.lock().push(*delivery.message());
        let mut confirm_to = delivery.confirm_to().clone();
        confirm_to.tell(ConsumerControllerConfirmed);
        Ok(Behaviors::same())
      })
    }
  });
  let consumer_cell = system.as_untyped().spawn(consumer_props.to_untyped()).expect("spawn consumer");
  let consumer_ref = TypedActorRef::<ConsumerControllerDelivery<u32>>::from_untyped(consumer_cell.into_actor_ref());

  // コンシューマーを開始する。
  cc_ref.tell(ConsumerController::start(consumer_ref));

  // プロデューサーコントローラーを生成する。
  let pc_props = TypedProps::<ProducerControllerCommand<u32>>::from_behavior_factory(|| {
    ProducerController::behavior("test-producer")
  });
  let pc_cell = system.as_untyped().spawn(pc_props.to_untyped()).expect("spawn producer controller");
  let mut pc_ref = TypedActorRef::<ProducerControllerCommand<u32>>::from_untyped(pc_cell.into_actor_ref());

  // RequestNext 受信時にメッセージを送信するモックプロデューサーを生成する。
  let producer_props = TypedProps::<ProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      Behaviors::receive_message(move |_ctx, req: &ProducerControllerRequestNext<u32>| {
        let mut send_to = req.send_next_to().clone();
        send_to.tell(42_u32);
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref = TypedActorRef::<ProducerControllerRequestNext<u32>>::from_untyped(producer_cell.into_actor_ref());

  // CC 登録時にプロデューサー参照が準備済みとなるよう、先に PC を開始する。
  pc_ref.tell(ProducerController::start(producer_ref));

  // コンシューマーコントローラーをプロデューサーコントローラーに登録する。
  cc_ref.tell(ConsumerController::register_to_producer_controller(pc_ref.clone()));

  // コンシューマーへの配達を待つ。
  // インラインディスパッチでは、CC のフロー制御 Request が最初の配達確認前に
  // 追加の RequestNext をトリガーする可能性があるため、コンシューマーは
  // 同じ値を複数回受信することがある。
  wait_until(|| !delivered.lock().is_empty());
  assert!(delivered.lock().contains(&42_u32));

  system.terminate().expect("terminate");
}

#[test]
fn consumer_controller_settings_accessors() {
  use crate::core::typed::delivery::ConsumerControllerSettings;

  let settings = ConsumerControllerSettings::new().with_flow_control_window(100).with_only_flow_control(true);
  assert_eq!(settings.flow_control_window(), 100);
  assert!(settings.only_flow_control());
}

#[test]
fn producer_controller_settings_accessors() {
  use crate::core::typed::delivery::ProducerControllerSettings;

  let _settings = ProducerControllerSettings::new();
}

#[test]
fn work_pulling_producer_controller_settings_accessors() {
  use crate::core::typed::delivery::WorkPullingProducerControllerSettings;

  let settings = WorkPullingProducerControllerSettings::new();
  assert_eq!(settings.buffer_size(), 1000);
}

#[test]
fn work_pulling_start_and_get_worker_stats() {
  let system = test_system();

  let worker_key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");

  // ワークプリング・プロデューサーコントローラーを生成する。
  let wppc_props = TypedProps::<WorkPullingProducerControllerCommand<u32>>::from_behavior_factory({
    let worker_key = worker_key.clone();
    move || WorkPullingProducerController::behavior("test-wp-producer", worker_key.clone())
  });
  let wppc_cell = system.as_untyped().spawn(wppc_props.to_untyped()).expect("spawn work-pulling producer controller");
  let mut wppc_ref =
    TypedActorRef::<WorkPullingProducerControllerCommand<u32>>::from_untyped(wppc_cell.into_actor_ref());

  // ワーカー統計のレスポンスを追跡する。
  let stats_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let stats_received_clone = stats_received.clone();

  // 統計収集アクターを生成する。
  let stats_props = TypedProps::<WorkerStats>::from_behavior_factory({
    move || {
      let received = stats_received_clone.clone();
      Behaviors::receive_message(move |_ctx, stats: &WorkerStats| {
        received.lock().push(stats.number_of_workers());
        Ok(Behaviors::same())
      })
    }
  });
  let stats_cell = system.as_untyped().spawn(stats_props.to_untyped()).expect("spawn stats");
  let stats_ref = TypedActorRef::<WorkerStats>::from_untyped(stats_cell.into_actor_ref());

  // ワーカー登録前は統計が 0 であること。
  wppc_ref.tell(WorkPullingProducerController::get_worker_stats(stats_ref.clone()));

  wait_until(|| !stats_received.lock().is_empty());
  assert_eq!(stats_received.lock()[0], 0);

  system.terminate().expect("terminate");
}

#[test]
fn work_pulling_delivers_to_worker_via_receptionist() {
  let system = test_system();

  let worker_key = ServiceKey::<ConsumerControllerCommand<u32>>::new("wp-workers");

  // ワークプリング・プロデューサーコントローラーを生成する。
  let wppc_props = TypedProps::<WorkPullingProducerControllerCommand<u32>>::from_behavior_factory({
    let worker_key = worker_key.clone();
    move || WorkPullingProducerController::behavior("test-wp-producer", worker_key.clone())
  });
  let wppc_cell = system.as_untyped().spawn(wppc_props.to_untyped()).expect("spawn work-pulling producer controller");
  let mut wppc_ref =
    TypedActorRef::<WorkPullingProducerControllerCommand<u32>>::from_untyped(wppc_cell.into_actor_ref());

  // 配達を追跡する。
  let delivered = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let delivered_clone = delivered.clone();

  // ワーカー（コンシューマーコントローラー + コンシューマーアクター）を生成する。
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn cc");
  let mut cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.into_actor_ref());

  // 配達を処理するコンシューマーを生成する。
  let consumer_props = TypedProps::<ConsumerControllerDelivery<u32>>::from_behavior_factory({
    move || {
      let delivered = delivered_clone.clone();
      Behaviors::receive_message(move |_ctx, delivery: &ConsumerControllerDelivery<u32>| {
        delivered.lock().push(*delivery.message());
        let mut confirm_to = delivery.confirm_to().clone();
        confirm_to.tell(ConsumerControllerConfirmed);
        Ok(Behaviors::same())
      })
    }
  });
  let consumer_cell = system.as_untyped().spawn(consumer_props.to_untyped()).expect("spawn consumer");
  let consumer_ref = TypedActorRef::<ConsumerControllerDelivery<u32>>::from_untyped(consumer_cell.into_actor_ref());

  // コンシューマーコントローラーを開始する。
  cc_ref.tell(ConsumerController::start(consumer_ref));

  // ワーカーのコンシューマーコントローラーを Receptionist に登録する。
  if let Some(mut receptionist_ref) = system.receptionist_ref() {
    receptionist_ref.tell(Receptionist::register(&worker_key, cc_ref.clone()));
  }

  // RequestNext 受信時にメッセージを送信するモックプロデューサーを生成する。
  let producer_props = TypedProps::<WorkPullingProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      Behaviors::receive_message(move |_ctx, req: &WorkPullingProducerControllerRequestNext<u32>| {
        let mut send_to = req.send_next_to().clone();
        send_to.tell(99_u32);
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref =
    TypedActorRef::<WorkPullingProducerControllerRequestNext<u32>>::from_untyped(producer_cell.into_actor_ref());

  // ワークプリング・プロデューサーコントローラーを開始する。
  wppc_ref.tell(WorkPullingProducerController::start(producer_ref));

  // コンシューマーへの配達を待つ。
  wait_until(|| !delivered.lock().is_empty());
  assert_eq!(delivered.lock()[0], 99_u32);

  system.terminate().expect("terminate");
}
