# Tick Driver Quickstart

Tick Driver の導入手順をまとめたハンドブックです。Tokio ランタイムで自動 Tick を流すケース（Task 5.1）から順に、組込み／テスト向けテンプレート（Task 5.2/5.3）を同じページで育てていきます。ここではまず std (Tokio) 版のクイックスタートを整備し、20 行未満の `main` 関数で Tick Driver を起動するまでを説明します。

## 1. std (Tokio) クイックスタート

### 1.1 手順の概要

1. **ドライバ構成の生成** — `StdTickDriverConfig::tokio_quickstart()` で 10ms 解像度の `TickDriverConfig<StdToolbox>` を一発生成する。（解像度を変えたい場合は `tokio_quickstart_with_resolution(Duration)` を呼ぶ）
2. **ブートストラップ** — `TickDriverBootstrap::provision(&config, &ctx)` を呼び、`SchedulerContext` に紐づいた `TickDriverRuntime`（driver + feed）を取得する。
3. **Executor ポンプの起動** — `SchedulerTickExecutor` を `tokio::spawn` で常駐させ、feed から tick を drain してスケジューラを駆動する。`feed.signal().wait_async().await` で背圧なく通知を受け取れる。
4. **検証** — `system.tick_driver_snapshot()` または EventStream (`EventStreamEvent::TickDriver`) を監視し、driver kind/resolution/auto メタデータが記録されていることを確認する。

### 1.2 Tokio テンプレート

```rust
use std::time::Duration;

use fraktor_actor_core_rs::{
  actor_prim::{Actor, ActorContext},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  scheduler::{SchedulerTickExecutor, TickDriverBootstrap},
};
use fraktor_actor_std_rs::{system::ActorSystem, tick::StdTickDriverConfig};

struct Start;
struct Guardian;

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessageView<'_>) -> anyhow::Result<()> {
    if msg.downcast_ref::<Start>().is_some() {
      println!("guardian started");
    }
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
  let system = ActorSystem::new(&Props::from_fn(|| Guardian))?;
  let ctx = system.scheduler_context().expect("scheduler context");
  let config = StdTickDriverConfig::tokio_quickstart();
  let runtime = TickDriverBootstrap::provision(&config, &ctx)?;
  let feed = runtime.feed().expect("feed").clone();
  let signal = feed.signal();

  tokio::spawn({
    let scheduler = ctx.scheduler();
    async move {
      let mut executor = SchedulerTickExecutor::new(scheduler, feed, signal.clone());
      loop {
        signal.wait_async().await;
        executor.drive_pending();
      }
    }
  });

  system.user_guardian_ref().tell(AnyMessage::new(Start))?;
  tokio::time::sleep(Duration::from_millis(250)).await;
  system.terminate()?;
  Ok(())
}
```

- `StdTickDriverConfig::tokio_quickstart()` が Task 5.1 で追加されたビルダーヘルパです。`TickDriverConfig::auto_with_factory(...)` の定型文をすべて内側に閉じ込め、Tokio 以外の設定は `tokio_quickstart_with_resolution` で差分指定できます。
- Executor ループは 10 行未満に収まるよう `signal.wait_async().await` → `drive_pending()` の最短ループを提示しています。Runner API を使わずにスケジューラが常時駆動されるため、本番構成でもそのまま流用できます。
- 起動後は `system.tick_driver_snapshot()` で driver kind / resolution を即座に確認できます。EventStream へも `EventStreamEvent::TickDriver` が publish されるので、監視面では LoggerSubscriber や `RecordingSubscriber` で簡単に検証可能です。

### 1.3 追加の検証ポイント

- **メトリクス**: `TickDriverBootstrap::provision` 時に TickFeed が自動的に起動し、1 秒毎の `SchedulerTickMetrics` が publish されます。Tokio Quickstart では `StdTickDriverConfig::tokio_quickstart()` の既定値（10ms・AutoPublish(1s)）が適用されます。
- **構成メタデータ**: `ActorSystem::tick_driver_snapshot()` が `TickDriverKind::Auto` と `AutoDriverMetadata::profile == AutoProfileKind::Tokio` を返すことをテストに組み込むと、Task 4.3 の要件（R3.6）を自動検証できます。
- **停止フロー**: `TickDriverBootstrap::shutdown(runtime.driver())` を `Drop` フックで呼び出すと、Tokio タスク／Timer が確実に停止し、`TickFeed::mark_driver_inactive()` で ISR/driver 側の状態遷移も閉じられます。

> **メモ**: 本ガイドは Task 5.2/5.3 で組込みテンプレートとテスト専用テンプレートを追記し、Driver 選択マトリクスと併せて全環境分のクイックスタート資料を 1 か所に集約する予定です。

## 2. 組込み (no_std) クイックスタート

### 2.1 ハードウェアドライバの取り付け手順

1. `static` な `TickPulseSource` 実装（SysTick, embassy Timer など）を用意する。
2. `TickDriverConfig::hardware(&PULSE)` で構成を作成し、`TickDriverBootstrap::provision` に渡す。
3. `SchedulerTickExecutor` を `TickExecutorPump` もしくは自前ループで駆動し、`feed.enqueue_from_isr` により ISR から届いた tick を drain する。
4. シャットダウン時は `TickDriverBootstrap::shutdown` を呼んで割り込みを停止し、`feed.mark_driver_inactive()` が実行されるようにする。

### 2.2 no_std テンプレート

```rust
use core::time::Duration;

use fraktor_actor_core_rs::{
  actor_prim::{Actor, ActorContext},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  scheduler::{SchedulerCommand, SchedulerTickExecutor, TickDriverBootstrap, TickDriverConfig},
  system::ActorSystem,
};

use my_board::tick::{SysTickPulse, SchedulerPump};

static SYS_TICK: SysTickPulse = SysTickPulse::new(/* timer peripherals */);

struct Guardian;

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessageView<'_>) -> anyhow::Result<()> {
    if msg.downcast_ref::<Start>().is_some() {
      let scheduler = ctx.system().scheduler_context().expect("scheduler").scheduler();
      scheduler.lock().schedule_once(
        Duration::from_millis(5),
        SchedulerCommand::SendMessage {
          receiver: ctx.self_ref(),
          message: AnyMessage::new(PulseAck),
          dispatcher: None,
          sender: None,
        },
      )?;
    }
    Ok(())
  }
}

#[entry]
fn main() -> ! {
  let system = ActorSystem::new(&Props::from_fn(|| Guardian)).expect("system");
  let ctx = system.scheduler_context().expect("scheduler");
  let config = TickDriverConfig::hardware(&SYS_TICK);
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("driver");
  let feed = runtime.feed().expect("feed").clone();
  let signal = feed.signal();

  let mut executor = SchedulerTickExecutor::new(ctx.scheduler(), feed, signal.clone());
  let mut pump = SchedulerPump::new(signal);
  loop {
    pump.wait_for_tick();
    executor.drive_pending();
  }
}
```

- `SchedulerPump` はボード依存（例: `embassy_executor::Spawner` や `critical_section` ベースの `wfe` ループ）で実装します。Task 5.2 の要件は「テンプレート内で attach〜executor の流れが明示されること」なので、上記のように `loop` 内へ 2 行押し込めば十分です。
- `embedded_quickstart_template_runs_ticks`（`modules/actor-core/src/scheduler/tick_driver/tests.rs`）では、`TestPulseSource` を使って上記テンプレートをそのままユニットテスト化しています。FIFO 順序を保ったまま Runnable が実行されること、ISR から enqueue した tick が `SchedulerTickExecutor` によって drain されることを保証します。
- ハードウェアを差し替える際は `SysTickPulse` を `EmbassyAlarmPulse` など別の `TickPulseSource` に置き換えるだけで済むよう、テンプレートにはドライバ固有の処理を一切書かない（コメントで差し替えポイントを明記）ことを推奨します。

## 3. テスト専用 (Manual) クイックスタート

> `ManualTestDriver` と Runner API は `#[cfg(any(test, feature = "test-support"))]` でのみコンパイルされます。プロダクションバイナリにはリンクされないため、テスト専用テンプレートも同じ `cfg` で囲んでください。

```rust
#![cfg(any(test, feature = "test-support"))]

use core::time::Duration;

use fraktor_actor_core_rs::{
  props::Props,
  scheduler::{SchedulerCommand, SchedulerTickExecutor, TickDriverBootstrap, TickDriverConfig},
  system::ActorSystem,
};

use fraktor_actor_core_rs::scheduler::ManualTestDriver;

#[test]
fn manual_driver_quickstart() {
  let driver = ManualTestDriver::new();
  let config = TickDriverConfig::manual(driver);
  let system = ActorSystem::new(&Props::from_fn(|| GuardianActor)).expect("system");
  let ctx = system.scheduler_context().expect("scheduler");

  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  assert!(runtime.feed().is_none());
  let controller = runtime.manual_controller().expect("controller");

  controller.inject_ticks(5);
  controller.drive();
  assert!(system.tick_driver_snapshot().is_some());
}
```

- Runner API は scheduler executor を生成しないため、`TickDriverRuntime::feed()` が `None` を返し、手動コントローラ (`ManualTickController`) の `drive()` メソッドで tick を前進させます。
- Task 5.3 のテスト (`manual_driver_runs_jobs_without_executor`) と上記サンプルはいずれも `cfg(test)` で閉じており、`ManualTestDriver` が本番ビルドへ混入しないことを自動検証しています。

## 4. Driver 選択マトリクス (TICK_DRIVER_MATRIX)

`modules/actor-core/src/scheduler/tick_driver/tick_driver_matrix.rs` で公開している `TICK_DRIVER_MATRIX` は、Quickstart ドキュメントと実装の両方から参照できるデータセットです。各エントリ（`TickDriverGuideEntry`）は kind / label / 既定解像度 / メトリクスモード / test-only フラグを保持し、以下の表と同じ情報を Rust API で取得できます。

| ラベル | kind | 既定解像度 | metrics | test-only | 説明 |
| --- | --- | --- | --- | --- | --- |
| auto-std | `TickDriverKind::Auto` | 10ms | AutoPublish(1s) | false | Tokio ランタイムを自動検出 (`StdTickDriverConfig::tokio_quickstart`) |
| hardware | `TickDriverKind::Hardware { source: Custom }` | 1ms | AutoPublish(1s) | false | `TickPulseSource` を attach する no_std テンプレート |
| manual-test | `TickDriverKind::ManualTest` | 10ms | OnDemand | true | Runner API / ManualTestDriver を使った決定論テスト専用 |

`driver_matrix_lists_auto_and_hardware_entries` / `driver_matrix_marks_manual_entry_as_test_only` の 2 テストで、表に記載したエントリが API の定義どおりであること、manual 行に `test_only = true` が付与されていることを常時チェックしています。
