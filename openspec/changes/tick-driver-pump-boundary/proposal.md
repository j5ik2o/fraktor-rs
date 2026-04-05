## Why

現在の tick driver 構成は `core` と `std` の責務境界が崩れている。

`modules/actor-adaptor/src/std/scheduler/tick.rs` は本来の adapter 実装に留まらず、`TickFeed`、`TickExecutorSignal`、`SchedulerTickExecutor`、`TickDriverHandle`、`TickDriverBundle`、自動ドライバーメタデータを自前で組み立てている。これは `std` adapter が `core` runtime の配線主体になっている状態であり、platform adapter の責務を超えている。

原因は `core::kernel::actor::scheduler::tick_driver::TickDriverConfig::new(...)` が「platform adapter を注入する API」ではなく、「完全な runtime bundle を組み立てる builder closure を注入する API」になっていることにある。結果として Tokio だけでなく showcase support 側にも同型の配線が重複している。

なお、`TickDriverConfig` という名前の型は `core` と `std` の両方に存在するが、再設計対象はあくまで `core::...::tick_driver::TickDriverConfig` である。`std` 側の `TickDriverConfig` は private helper であり、設計上の主語にはしない。

この change では、tick driver 実行配線の構築責務を `core` へ戻し、adapter は tick source / executor pump の最小実装だけを提供する境界へ再設計する。

## What Changes

- `core` が `TickFeed`、`TickExecutorSignal`、`SchedulerTickExecutor`、`TickDriverHandle`、`TickDriverBundle`、自動ドライバーメタデータを組み立てる責務を持つ
- `core` 側の `TickDriverConfig` を「完全な bundle を返す builder」から「platform adapter / executor pump を指定する設定」へ再定義する
- Tokio adapter は `std` 側で tick source と executor pump の実装のみを提供する
- showcase support の独自 tick runtime 配線を `core` API 利用へ寄せる
- `TickDriverFactory` / `TickDriver` / `TickPulseSource` / `TickExecutorPump` の役割を整理し、どの abstraction を正規の注入点にするかを固定する
- tick driver のプロビジョニング失敗は `TickDriverError` として返し、adapter 内部で platform 前提不足を安易に panic しない契約へ寄せる
- `ActorSystem::new()` のデフォルト tick driver 構成は維持しつつ、内部 wiring だけを `core` 主体へ移す

## Capabilities

### Modified Capabilities
- `actor-system-default-config`: デフォルト tick driver 構成は維持しつつ、tick driver 実行配線を `core` 主体へ切り替える

### New Capabilities
- `tick-driver-runtime-boundary`: platform adapter が `core` の内部配線を握らずに tick driver を提供できる

## Impact

- 影響コード:
  - `modules/actor/src/core/kernel/actor/scheduler/tick_driver/*`
  - `modules/actor-adaptor/src/std/scheduler/tick.rs`
  - `showcases/std/src/support/tick_driver.rs`
  - `modules/actor-adaptor/src/std/system/base.rs`
- 影響 API:
  - `core::...::TickDriverConfig`
  - `TickDriverBundle`
  - `TickDriverFactory` / `TickDriver` / `TickPulseSource`
  - `TickExecutorPump`（新設）
  - std 側 default tick driver helper
- 互換性:
  - 後方互換は不要
  - 破壊的変更を許容して責務境界を正す
