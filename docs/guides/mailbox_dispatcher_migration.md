# Mailbox / Dispatcher 移行ガイド

> 最終更新: 2025-11-11

Pekko 互換の Mailbox / Dispatcher 構成を段階的に導入するための手順と検証ポイントを整理します。no_std / std 共通 API を維持しつつ、SystemQueue・Backpressure・ScheduleAdapter・Telemetry を同時に移行できるように設計されています。

## フェーズ別の適用手順

1. **基盤フェーズ (P1)**  
   - `config::dispatchers` / `config::mailboxes` へ既存設定を登録し、Props 側では ID で参照するだけにします。  
   - `QueueCapabilityRegistry` で Stash 要件や Block 戦略が満たされているかを `SpawnError` 経由で即検知できるようにします。

2. **バックプレッシャーフェーズ (P2)**  
   - Mailbox instrumentation に `warn_threshold` と `BackpressurePublisher` を設定し、EventStream へ `MailboxPressureEvent` をPublish できる状態にします。  
   - Dispatcher 側では `ScheduleHints.backpressure_active` を受け取り、registerForExecution 頻度を落とす/増やすチューニングを有効化します。

3. **スケジューラフェーズ (P3)**  
   - `ScheduleAdapter` を runtime ごとに差し替え、tokio / inline / no_std executor で同一の waker 契約を共有します。  
   - `MailboxOfferFuture` の Block 戦略では `with_timeout` を利用し、RejectedExecution ルートは Adapter 経由で EventStream に通知します。

4. **可観測性フェーズ (P4)**  
   - Dispatcher 経由で `publish_dump_metrics` を呼び出し、`EventStreamEvent::DispatcherDump` をサブスクライバへ提供します。  
   - `docs/guides/mailbox_dispatcher_migration.md` で該当ステップをチェックしつつ、CI では `./scripts/ci-check.sh all` で no_std / std 双方の回帰を確認します。

## 検証とテストカバレッジ

- `mailbox/state_engine/tests.rs::backpressure_hint_requests_schedule_when_not_suspended`  
  Backpressure ヒントでスケジューリングが正しく再通知されることを保証します。

- `mailbox/state_engine/tests.rs::backpressure_hint_is_ignored_while_suspended`  
  Suspend 中は backpressure だけでは再スケジュールされないことを明示し、誤作動を防ぎます。

- `modules/actor-core/tests/telemetry_pipeline.rs::mailbox_pressure_and_dispatcher_dump_are_published`  
  Mailbox→Dispatcher→EventStream のルートで `MailboxPressure` と `DispatcherDump` が同時に観測できることを E2E で確認します。

これらに加えて既存の `dispatcher::tests::*` や `event_stream` 系統の統合テストが SystemQueue / ScheduleAdapter / DeadLetter の回帰を防いでいます。新機能を有効化した後は、**必ず `./scripts/ci-check.sh all` を通過**させてからローリングリリースしてください。
