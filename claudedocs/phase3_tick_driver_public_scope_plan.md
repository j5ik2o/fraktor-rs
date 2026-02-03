# Phase 3: tick_driver 公開範囲整理計画

## 目的

tick_driver のドメインプリミティブを維持したまま、不要な公開 API を整理する。

## 方針

- 列挙型や識別子などのドメインプリミティブは統合・数値化しない
- 公開範囲の整理は `pub(crate)` 化と再エクスポート整理に限定する
- 既存のドメイン境界は維持し、境界でのみプリミティブへ変換する

## 公開維持（ドメインプリミティブ）

- `TickDriverId`
- `TickDriverKind`
- `HardwareKind`
- `AutoProfileKind`
- `AutoDriverMetadata`
- `TickDriverMetadata`
- `SchedulerTickMetrics`

## 公開維持（構築/拡張 API）

- `TickDriverConfig`
- `TickDriver`
- `TickDriverControl`
- `TickDriverError`
- `TickDriverBundle`
- `TickDriverHandleGeneric`
- `TickFeed` / `TickFeedHandle`
- `TickExecutorSignal`
- `TickDriverProvisioningContext`
- `TickPulseSource` / `TickPulseHandler`
- `HardwareTickDriver`
- `SchedulerTickExecutor`

## 公開整理候補（pub(crate) 化）

- `TickDriverGuideEntry`
- `TICK_DRIVER_MATRIX`
- `TickMetricsMode`
- `SchedulerTickHandleOwned`
- `SchedulerTickMetricsProbe`
- `TickDriverBootstrap`
- `next_tick_driver_id`
- `TickDriverHandle`

## 作業手順

1. `modules/actor/src/core/scheduler.rs` の再エクスポートを整理し、公開維持と `pub(crate)` を分離する
2. 影響範囲を `rg` で確認し、内部利用はそのまま維持する
3. 変更後に `./scripts/ci-check.sh all` を実行し、テストを通す

## 影響

公開整理候補は外部クレートから参照できなくなる。現状の参照は `modules/actor` 内のみのため、影響は限定的と見込む。
