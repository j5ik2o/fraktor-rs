## 1. 現状分析と境界固定

- [x] 1.1 `core::...::TickDriverConfig::Builder` の利用箇所を棚卸しし、complete bundle builder に依存している箇所を一覧化する
- [x] 1.2 `TickDriver` / `TickPulseSource` / `TickDriverFactory` / `TickExecutorPump` の役割整理を design に反映し、`TickDriverBundle` / `SchedulerTickExecutor` / `TickFeed` / `TickDriverHandle` の構築責務を `core` に戻す設計境界を固定する

## 2. Core Tick 配線 再設計

- [x] 2.1 `core` 側に `TickDriver` + `TickExecutorPump` を配線する API を追加する
- [x] 2.2 `core` 側 `TickDriverConfig` から complete bundle builder 契約を除去する
- [x] 2.3 `TickDriverFactory` の利用箇所を確認し、外部公開が不要なら削除、必要でも crate-private へ縮小して正規の注入点から外す
- [x] 2.4 `TickDriverBundle::with_executor_shutdown(...)` を削除し、executor lifecycle を `core` が所有する形へ置き換える
- [x] 2.5 tick driver adapter 内の `expect(...)` / panic 前提を棚卸しし、`TickDriverError` へ寄せる

## 3. Adapter 最小化

- [x] 3.1 Tokio adapter を `TickDriver` 実装と `TickExecutorPump` 実装の最小責務へ分割する
- [x] 3.2 `modules/actor-adaptor/src/std/scheduler/tick.rs` を `core` tick driver 配線 API を使う実装へ置き換える
- [x] 3.3 `showcases/std/src/support/tick_driver.rs` の重複 wiring を新 API 利用へ置き換える

## 4. 検証

- [x] 4.1 tick driver 関連の core / std / showcase tests を更新する
- [x] 4.2 `ActorSystem::new()` のデフォルト tick driver 構成が維持されることを確認する
- [x] 4.3 `TickDriverBundle::new(...)` などの内部構築 API が `std` / showcase 側に再侵入していないことを確認する
- [x] 4.4 tick driver プロビジョニング失敗が panic ではなく `TickDriverError` に落ちることを確認する
- [x] 4.5 `./scripts/ci-check.sh ai all` で最終確認する
