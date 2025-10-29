---

description: "セルアクター no_std ランタイム初期版の実装タスクリスト"

---

# タスク: セルアクター no_std ランタイム初期版

**入力**: `/specs/001-add-actor-runtime/` 配下の設計ドキュメント  
**前提条件**: plan.md（必須）、spec.md（ユーザーストーリー参照）、research.md、data-model.md、contracts/

**テスト方針**: 原則2に従い、ユーザーストーリー単位で独立した検証ができるようにする。`modules/actor-core/tests/` にストーリー別の統合テストを追加し、`cfg(test)` 下でのみ `std` を有効化する。実装前に既存コードの設計パターン（1ファイル1構造体／trait、`ArcShared` 抽象、`no_std` 運用）を確認し、乖離する場合は理由と影響を記録する。共有参照・ロックは必ず `modules/utils-core` の抽象 (`Shared`/`ArcShared`, `Async/SyncMutexLike`) を利用し、`alloc::sync::Arc` へ直接依存しない。API とデータフローは借用ベースのライフタイム設計を採り、ヒープ確保は不可避な箇所に限定して計測・再利用戦略をタスク内で明示する。`sender()` は導入せず、メッセージの `reply_to: ActorRef` を必須パターンとする。作業の節目ごとに `./scripts/ci-check.sh all` と `makers ci-check -- dylint` を実行し、失敗時はログを残す。  
**構成**: タスクはユーザーストーリーごとにグルーピングし、依存関係が無いものは `[P]` で並列実行可とする。

## 形式: `[ID] [P?] [Story] 説明`

- **[P]**: 依存のない並列実行可タスク  
- **[Story]**: 対応するユーザーストーリー（例: [US1], [US2]）  
- 説明には正確なファイルパスを記載すること

## パス規約

- 中心クレート: `modules/actor-core`, `modules/utils-core`  
- 例示用コード: `examples/` 配下  
- 契約: `specs/001-add-actor-runtime/contracts/actor-system.openapi.yaml`  
- 単体／統合テスト: `modules/<crate>/tests/`

---

## フェーズ1: セットアップ（共通基盤）

**目的**: ワークスペース・依存関係・CI を準備し、`modules/actor-core` が `#![no_std]` で動作する土台を整える。

- [x] T001 Update workspace manifest to expose `modules/actor-core` features and default flags (Cargo.toml)
- [x] T002 Align `modules/actor-core/Cargo.toml` dependencies (`portable-atomic`, `heapless`, `portable-atomic-util`, `modules/utils-core`) for no_std + alloc support (modules/actor-core/Cargo.toml)
- [x] T003 Configure crate root with `#![no_std]`, module declarations, and shared re-exports (modules/actor-core/src/lib.rs)
- [x] T004 Extend CI pipeline to run `cargo check --no-default-features --package actor-core` (scripts/ci-check.sh)

---

## フェーズ2: 基盤整備（全ストーリーに必須）

**目的**: すべてのストーリーで共有するコア抽象（Actor/Context/Error/Message など）を定義する。

- [x] T005 Define `Actor` trait with `pre_start` / `receive` / `post_stop` lifecycle signatures (modules/actor-core/src/actor.rs)
- [x] T006 Implement `ActorContext` struct scaffolding（self PID、spawn hooks、reply helpers）(modules/actor-core/src/actor_context.rs)
- [x] T007 Add `ActorError` enum with `Recoverable` / `Fatal` variants and helper constructors (modules/actor-core/src/actor_error.rs)
- [x] T008 Implement `AnyMessage` wrapper with type-id metadataとdowncastユーティリティ (modules/actor-core/src/any_message.rs)
- [x] T009 Provide polling-based `ActorFuture` skeleton with completion callbacks (modules/actor-core/src/actor_future.rs)
- [x] T010 Define `Pid` structure and O(1) registry keys (modules/actor-core/src/pid.rs)
- [x] T011 Implement `NameRegistry` for parent-scoped unique names + auto `anon-{pid}` generation (modules/actor-core/src/name_registry.rs)
- [x] T012 Create `ReceiveState` state machine supporting become/unbecome stack (modules/actor-core/src/receive_state.rs)
- [x] T013 Declare `SupervisorStrategy` data structures（OneForOne / AllForOne / decider）(modules/actor-core/src/supervisor_strategy.rs)
- [x] T014 Add `Props` builder, `MailboxConfig`, `SupervisorOptions` definitions (modules/actor-core/src/props.rs)
- [x] T015 Define `MailboxPolicy` and capacity strategy enums covering DropNewest/DropOldest/Grow/Block + Bounded/Unbounded flags (modules/actor-core/src/mailbox_policy.rs)

---

## フェーズ3: ユーザーストーリー 1（優先度: P1） 🎯 MVP

**目標**: AnyMessage を使った最小構成でアクターを起動し、Ping/Pong サンプルが no_std + alloc 環境で動作する。  
**独立テスト**: `modules/actor-core/tests/ping_pong.rs` で spawn / tell / 背圧ポリシー / reply_to 処理が通ること。

- [x] T016 [US1] Implement `ActorRef` handle with未型付けの `tell`/`ask` APIs and ArcShared storage（`AnyOwnedMessage` を受け付け、戻り値で送信失敗を検知できる）(modules/actor-core/src/actor_ref.rs)
- [x] T017 [US1] Implement `Mailbox` struct supporting DropNewest/DropOldest/Grow policies and Bounded/Unbounded capacity (modules/actor-core/src/mailbox.rs)
- [x] T018 [US1] Implement `Dispatcher` with throughput limiting and scheduling hooks (modules/actor-core/src/dispatcher.rs)
- [x] T019 [US1] Implement `MessageInvoker` pipeline executing middleware chain and reply_to routing (modules/actor-core/src/message_invoker.rs)
- [x] T020 [US1] Implement `ActorSystem` core（guardian Props、`user_guardian_ref()`、name registry、`spawn_child` 経由の生成、reply_to dispatch）(modules/actor-core/src/system.rs)
- [ ] T021 [US1] Complete `ActorFuture` ask helpers tying into ActorSystem (modules/actor-core/src/actor_future.rs)
- [x] T022 [P] [US1] Add no_std Ping/Pong example showcasing AnyMessage + reply_to (examples/ping_pong_no_std/main.rs)
- [x] T023 [P] [US1] Add integration tests for spawn/tell/backpressure/auto naming (modules/actor-core/tests/ping_pong.rs)

---

## フェーズ4: ユーザーストーリー 2（優先度: P2）

**目標**: 親子アクターの監督ツリーを構築し、Supervisor 戦略に基づく再起動／停止を実現する。  
**独立テスト**: `modules/actor-core/tests/supervisor.rs` で Restart/Escalate ポリシーと子アクター監視が検証できること。

- [ ] T024 [US2] Implement `RestartStatistics` tracker for rate-limited restarts (modules/actor-core/src/restart_statistics.rs)
- [ ] T025 [US2] Wire `SupervisorStrategy` decision logic with Restart/Fatal/Escalate handling (modules/actor-core/src/supervisor_strategy.rs)
- [ ] T026 [US2] Extend `ActorContext` with `spawn_child`, child registry, and supervision signals (modules/actor-core/src/actor_context.rs)
- [ ] T027 [US2] Connect `ActorSystem` to maintain supervisor tree and propagate failures upward (modules/actor-core/src/system.rs)
- [ ] T028 [US2] Add `ChildRef` wrapper to manage child handles and lifecycle hooks (modules/actor-core/src/child_ref.rs)
- [ ] T029 [P] [US2] Add supervision regression tests covering Restart/Escalate + panic 非介入 (modules/actor-core/tests/supervisor.rs)

---

## フェーズ5: ユーザーストーリー 3（優先度: P3）

**目標**: EventStream / Deadletter / Logger によるオブザーバビリティとホスト制御面を提供する。  
**独立テスト**: `modules/actor-core/tests/event_stream.rs` で LogEvent 配信・Deadletter 記録・容量警告が検証できること。

- [ ] T030 [US3] Implement `EventStream` publish/subscribe bus with buffered delivery (modules/actor-core/src/event_stream.rs)
- [ ] T031 [US3] Implement `Deadletter` store with EventStream forwarding (modules/actor-core/src/deadletter.rs)
- [ ] T032 [US3] Implement `LoggerSubscriber` that routes LogEvent to UART/RTT hooks (modules/actor-core/src/logger_subscriber.rs)
- [ ] T033 [US3] Emit lifecycle/log events from ActorSystem/Supervisor paths (modules/actor-core/src/system.rs)
- [ ] T034 [US3] Instrument Mailbox to emit capacity warnings and throughput metrics (modules/actor-core/src/mailbox.rs)
- [ ] T035 [US3] Provide host-control shim matching OpenAPI contract (contracts/actor-system.openapi.yaml, examples/host_control_std/main.rs)
- [ ] T036 [P] [US3] Add integration tests for EventStream + Deadletter flows (modules/actor-core/tests/event_stream.rs)
- [ ] T037 [P] [US3] Add logger subscriber example demonstrating LogEvent consumption (examples/logger_subscriber_std/main.rs)

---

## フェーズ6: 仕上げ・横断対応

**目的**: ドキュメント整備・性能検証・最終 CI を実施する。

- [ ] T038 Update runtime guide with usage, reply_to パターン、監視手順 (docs/guides/actor-system.md)
- [ ] T039 Add throughput benchmark harness for mailbox/dispatcher (modules/actor-core/tests/perf_mailbox.rs)
- [ ] T040 Update Makefile recipes to include actor-core story pipelines and final CI target (Makefile.toml)

---

## 依存関係と実行順序

- フェーズ1 → フェーズ2 → US1 → US2 → US3 → フェーズ6
- US1 完了が US2 / US3 の前提。US2 と US3 はそれぞれ独立テストが通ったあとフェーズ6へ進む。

## 並列実行の例

- US1: T022 と T023 は T020 完了後に並列実行可。  
- US2: T024・T025 完了後に T029 を並列で進められる。  
- US3: T036 と T037 は T033 まで完了していれば同時着手可。  
- フェーズ6: T038 と T039 は実装完了後に並列実行し、最後に T040 で仕上げ。

## 実装戦略

1. **MVP (US1)**: ActorSystem、ActorRef、Mailbox、Dispatcher、MessageInvoker を最小構成で完成させ、Ping/Pong サンプルと統合テストを通す。  
2. **信頼性 (US2)**: RestartStatistics・SupervisorStrategy・子アクター監視を追加し、panic 非介入ポリシーとイベント通知を確立する。  
3. **オブザーバビリティ (US3)**: EventStream/Deadletter/Logger を導入し、OpenAPI ベースのホスト制御面を提供する。  
4. **Polish**: ドキュメント／ベンチマーク／CI を整え、no_std + alloc での運用を確実にする。
