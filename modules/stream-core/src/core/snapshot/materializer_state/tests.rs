extern crate std;

use core::time::Duration;

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use crate::core::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
  snapshot::MaterializerState,
};

// --- テスト用 ActorSystem 構築ヘルパー ---

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_with_config(&props, config).expect("system should build")
}

fn build_running_materializer() -> ActorMaterializer {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  materializer
}

// --- テスト観点1: 空 materializer で空 Vec ---

#[test]
fn stream_snapshots_on_idle_materializer_is_empty() {
  // Given: start されていない Idle 状態の materializer
  let system = build_system();
  let materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  // When: stream_snapshots を取得
  let snapshots = MaterializerState::stream_snapshots(&materializer);

  // Then: 未起動のため空 Vec
  assert!(snapshots.is_empty());
}

#[test]
fn stream_snapshots_on_running_materializer_without_materialize_is_empty() {
  // Given: start 済みだが materialize を一度も呼んでいない Running 状態の materializer
  let materializer = build_running_materializer();

  // When: stream_snapshots を取得
  let snapshots = MaterializerState::stream_snapshots(&materializer);

  // Then: ハンドル未登録のため空 Vec
  assert!(snapshots.is_empty());
}

// --- テスト観点2: 1 つ materialize 後の Vec 長 1 + logics 非空 ---

#[test]
fn stream_snapshots_after_single_materialize_has_length_one() {
  // Given: Running 状態の materializer
  let mut materializer = build_running_materializer();

  // When: 単一アイランドのグラフを 1 つ materialize
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");
  let snapshots = MaterializerState::stream_snapshots(&materializer);

  // Then: 登録ハンドル 1 つに対応するスナップショットが 1 件
  assert_eq!(snapshots.len(), 1);
}

#[test]
fn stream_snapshots_after_single_materialize_contains_non_empty_logics() {
  // Given: Running 状態の materializer + 単一アイランドグラフを materialize
  let mut materializer = build_running_materializer();
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  // When: stream_snapshots を取得
  let snapshots = MaterializerState::stream_snapshots(&materializer);

  // Then: 少なくとも 1 つの interpreter（active または new_shells）が
  // 非空の logics を持つ（Pekko `logics` 非空と等価）
  let snapshot = snapshots.first().expect("snapshot should exist");
  let has_active_logics = snapshot.active_interpreters().iter().any(|interp| {
    use crate::core::snapshot::InterpreterSnapshot;
    !interp.logics().is_empty()
  });
  let has_shell_logics = snapshot.new_shells().iter().any(|shell| {
    use crate::core::snapshot::InterpreterSnapshot;
    !shell.logics().is_empty()
  });
  assert!(has_active_logics || has_shell_logics, "expected at least one interpreter to carry logics");
}

// --- テスト観点3: 2 つ連続 materialize 後の Vec 長 2 ---

#[test]
fn stream_snapshots_after_two_sequential_materializes_has_length_two() {
  // Given: Running 状態の materializer
  let mut materializer = build_running_materializer();

  // When: 2 つの単一アイランドグラフを順次 materialize
  let graph1 = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _m1 = graph1.run(&mut materializer).expect("materialize 1");
  let graph2 = Source::single(2_u32).into_mat(Sink::head(), KeepRight);
  let _m2 = graph2.run(&mut materializer).expect("materialize 2");

  // Then: ハンドル 2 つに対応して 2 件のスナップショット
  let snapshots = MaterializerState::stream_snapshots(&materializer);
  assert_eq!(snapshots.len(), 2);
}

// --- テスト観点4: shutdown 後に空 Vec ---

#[test]
fn stream_snapshots_after_shutdown_is_empty() {
  // Given: materialize 後に shutdown した materializer
  let mut materializer = build_running_materializer();
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  // When: shutdown を発行
  materializer.shutdown().expect("shutdown");
  let snapshots = MaterializerState::stream_snapshots(&materializer);

  // Then: handles はクリアされ空 Vec を返す
  assert!(snapshots.is_empty());
}

// --- テスト観点5: マルチアイランド materialize 後にアイランド数分の snapshot ---

#[test]
fn stream_snapshots_after_multi_island_materialize_matches_island_count() {
  // Given: Running 状態の materializer
  let mut materializer = build_running_materializer();

  // When: `r#async()` で最終ノードを async 境界としてマークし、2 アイランド
  // 構成のグラフを materialize する。Source(async) → Flow(map) → Sink で
  // Island 1: [Source], Island 2: [Flow, Sink] の 2 islands / 1 crossing が
  // 期待される。
  //
  // NOTE: 旧 `async_boundary()` は deprecated かつ attribute を付けないため
  // `IslandSplitter` が境界を認識せず 1 island に畳まれてしまう。テスト意図
  // (マルチアイランド実装の検証) を満たすため、正しい API `r#async()` を使う。
  let graph = Source::single(1_u32).r#async().map(|value| value + 1).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize multi-island");

  // Then: アイランド数と同じ 2 件のスナップショットが返る
  let snapshots = MaterializerState::stream_snapshots(&materializer);
  assert_eq!(snapshots.len(), 2);
}
