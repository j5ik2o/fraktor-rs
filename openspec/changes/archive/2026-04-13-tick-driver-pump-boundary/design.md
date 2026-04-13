## Context

tick driver まわりには 2 種類の abstraction が同居している。

1. `TickDriver` / `TickDriverFactory` / `TickPulseSource`
2. `core::kernel::actor::scheduler::tick_driver::TickDriverConfig::new(builder)` による完全な bundle builder

加えて `modules/actor-adaptor/src/std/scheduler/tick.rs` に private helper として同名の `TickDriverConfig` があるが、これは設計上の注入点ではなく、`core` 側 API の不自然さを吸収するための一時的な wrapper に過ぎない。本 change の主対象は `core` 側である。

後者が強すぎるため、adapter が `core` の tick driver 実行配線を再構成する余地を持ち、Tokio adapter や showcase support が `TickFeed` と `SchedulerTickExecutor` の配線を複製している。

本来 `core` は scheduler runtime の構成主体であり、platform adapter は「tick をどう発生させるか」「executor をどう駆動するか」だけ知っていればよい。この change はその責務分離を回復する。

### 現行利用箇所の棚卸し

`TickDriverConfig::Builder` による complete bundle builder の利用は、現時点では以下に限られる。

- production
  - `modules/actor-adaptor/src/std/scheduler/tick.rs`
    - `TickDriverConfig::default_config()`
    - `TickDriverConfig::with_resolution(...)`
  - `showcases/std/src/support/tick_driver.rs`
    - `hardware_tick_driver_config_with_handle(...)`
    - `tokio_tick_driver_config_with_resolution(...)`
- test-only
  - `modules/actor/src/core/kernel/actor/scheduler/tick_driver/tests.rs`
    - `hardware_test_config(...)`
  - `modules/actor/src/core/kernel/actor/scheduler/tick_driver/tick_driver_config/tests.rs`
    - `test_tick_driver_config_builder()`
  - `modules/actor/src/core/kernel/system/base/tests.rs`
  - `modules/actor/src/core/kernel/system/state/system_state/tests.rs`

また、`TickDriverBundle::with_executor_shutdown(...)` の production 利用は `showcases/std/src/support/tick_driver.rs` の hardware path のみで、`TickDriverFactory` の非テスト参照は存在しない。

## Goals / Non-Goals

**Goals:**
- `core` が tick driver 実行配線を所有する
- adapter が runtime bundle の内部構成を知らずに済む
- Tokio / thread / hardware / 将来の他 platform で共通に使える abstraction に整える

**Non-Goals:**
- scheduler 自体のアルゴリズム変更
- tick metrics や metadata の意味論変更
- `ActorSystem::new()` の default 挙動変更

## Decisions

### 1. `core` 側の `TickDriverConfig` は complete bundle builder をやめる

`core` 側の `TickDriverConfig::new(|ctx| -> TickDriverBundle { ... })` は責務を持ちすぎている。これをやめ、config は platform-specific adapter を指定する設定へ縮小する。

この change では以下を決定する。

- `TickDriver` は残す
  - 役割: `TickFeedHandle` へ tick を供給する source driver 契約
- `TickPulseSource` は残す
  - 役割: hardware / 割り込み系 source を `TickDriver` 化する低レベル helper
- `TickDriverFactory` は正規の注入点から外す
  - builder closure 時代の周辺 abstraction であり、runtime 境界の主契約にはしない
  - 現行実装では定義ファイルと `tick_driver.rs` の再公開以外に参照がないため、この change の最初のバッチで削除する
- `TickExecutorPump` を新設する
  - 役割: `TickExecutorSignal` を待機し、`SchedulerTickExecutor` を platform runtime 上で駆動する

`TickDriverConfig` は最終的に「driver source」と「executor pump」を指定する設定へ置き換える。型名の最終形は実装時に合わせるが、責務の分離は上記で固定する。

つまり、complete bundle builder は廃止し、`TickDriver` + `TickExecutorPump` の組み合わせを `core` が配線する。

### 2. `core` が executor と feed を組み立てる

`TickFeed`、`TickExecutorSignal`、`SchedulerTickExecutor`、`TickDriverHandle`、`TickDriverBundle`、自動ドライバーメタデータは `core` が構築する。adapter はこれらを組み立てない。

これにより、`TickDriverBundle::with_executor_shutdown(...)` のような「外部で executor を作った前提の API」は不要になる。実装では削除を前提とする。

### 3. adapter は `TickDriver` と `TickExecutorPump` を実装する

platform ごとの差は本質的に以下の 2 点である。

- tick source をどう動かすか
- executor をどの runtime で待機 / 駆動するか

このため、設計の基準線は以下の責務分割とする。

- `TickDriver`
  - tick source の責務を担う
- `TickExecutorPump`
  - executor の待機 / 駆動責務を担う
  - 想定シグネチャは「`TickExecutorSignal` と `SchedulerTickExecutor` を受け取り、platform 上で待機しつつ `drive_pending()` を繰り返す」方向とする
  - Tokio のような async runtime と thread/polling 実装を両立できるよう、実装形は trait object か enum variant かを Open Questions で詰める

Tokio の場合は `tokio::time::interval` を使う `TickDriver` 実装と、`tokio::spawn` / `signal.wait_async()` を使う `TickExecutorPump` 実装になる。hardware の場合は `TickPulseSource` から `TickDriver` を作り、executor 側は polling/thread など別実装を割り当てる。

### 4. showcase support の wiring 重複は core API 利用へ置き換える

`showcases/std/src/support/tick_driver.rs` に存在する hardware / Tokio 向け tick driver 配線は、再設計後の `core` API を使う形へ寄せる。showcase が `TickFeed` や `SchedulerTickExecutor` を直接組み立てる状態は残さない。

### 5. デフォルト構成の利用体験は維持する

`ActorSystem::new()` が `tokio-executor` feature 有効時にデフォルト tick driver 構成で起動できる利用体験は維持する。今回変えるのは内部 wiring の責務だけであり、利用者の入口は壊さない。

### 6. tick driver のプロビジョニング失敗は `TickDriverError` に寄せる

platform 前提不足や adapter 初期化失敗は、可能な限り `TickDriverError` として表現する。少なくとも tick driver adapter の内部で `expect(...)` によって context 不足を即 panic する設計は避ける。

ただし、`ActorSystem::new()` 全体の利用体験は別 capability でも管理されているため、システム全体として panic を維持するかどうかは `actor-system-default-config` の更新と合わせて扱う。

## Risks / Trade-offs

- `TickDriverConfig` の破壊的変更により、既存 helper の呼び出し側更新が必要になる
- hardware path と async runtime path を同じ abstraction で表現するため、型設計が中途半端だと再び builder escape hatch が必要になる
- `core` が tick driver 配線を所有すると、抽象化を誤ると逆に platform ごとの差分を隠しきれない可能性がある

## Migration Plan

1. 現行 `TickDriverConfig::Builder` 依存箇所を棚卸しする
2. `core` 側に tick driver 配線 API を追加する
3. Tokio adapter を最小 contract へ移す
4. showcase support を新 API に追随させる
5. `ActorSystem::new()` とデフォルト tick driver 構成が引き続き動くことを確認する
6. builder closure ベース API を削除する

## Open Questions

- `TickExecutorPump` を trait object で保持するか、enum variant で保持するか
- hardware path における executor 側を no-op / polling / thread のどれで標準化するか
