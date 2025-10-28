# タスク: Cellactor Actor Core 初期実装

**入力**: `/specs/002-init-actor-lib/` 配下の設計ドキュメント（plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md）  
**前提条件**: プロジェクト憲章および plan.md の技術方針を遵守し、`modules/*-core` は `#![no_std]` を維持する。

**テスト方針**: 原則2に従い、各ユーザーストーリーで失敗するテストを先行実装する。テストは `modules/actor-core/tests/` 配下に作成し、`std` 依存は `cfg(test)` 内に限定する。作業の節目ごとに `./scripts/ci-check.sh all` と `makers ci-check -- dylint` を実行する。

## フェーズ1: セットアップ（共通基盤）

**目的**: 新規モジュールを追加する準備と CI 基盤の確認を行う。

- [ ] T001 更新対象モジュールのエントリを追加し `modules/actor-core/src/lib.rs` で新規ファイルを宣言する
- [ ] T002 `modules/actor-core/Cargo.toml` に `portable-atomic` と `heapless` 依存を宣言し `no_std` 構成を確認する
- [ ] T003 [P] `modules/actor-core/tests/common.rs` を作成しテスト用ユーティリティ基盤を整える
- [ ] T004 `./scripts/ci-check.sh` を用いて `./scripts/ci-check.sh all` のベースラインを取得し結果を記録する

## フェーズ2: 基盤整備（全ストーリーに必須）

**目的**: すべてのストーリーで利用する共通データ型と観測基盤を実装する。

- [ ] T101 `modules/actor-core/src/observation_channel.rs` に `ObservationChannel<T>` と `ObservationMode` を実装する
- [ ] T102 [P] `modules/actor-core/src/message_queue_policy.rs` に `MessageQueuePolicy` と関連列挙を定義する
- [ ] T103 `modules/actor-core/src/backpressure_hint.rs` に `BackpressureHint` を実装し Mailbox/EventStream で共有する
- [ ] T104 [P] utils-core キュー調査結果を追記し `specs/002-init-actor-lib/research.md` にバックプレッシャー整理を追加する
- [ ] T105 `modules/actor-core/src/system_id.rs` に `SystemId` 新規定義を追加する
- [ ] T106 [P] `modules/actor-core/src/scope_id.rs` に `ScopeId` 新規定義を追加する
- [ ] T107 `modules/actor-core/src/execution_runtime/mod.rs` に `ExecutionRuntime` トレイトと `ExecutionRuntimeRegistry` を定義し、CoreSync をデフォルト登録する
- [ ] T108 [P] `modules/actor-core/src/execution_runtime/core_sync.rs` に CoreSync 実装を追加し、ReadyQueueCoordinator/DispatcherRuntime を駆動するループを提供する
- [ ] T109 `modules/actor-core/tests/execution_runtime/tests.rs` に CoreSync ランタイムが ActorSystem 起動時に自動登録されることを検証する

## フェーズ3: ユーザーストーリー1（優先度: P1） — システム内で安全にアクターを起動したい 🎯

**目標**: `ActorSystem::with_scope` によりスコープ内でのみ利用可能な `ActorRef`/`ActorContext` を提供し、参照流出を防ぐ。  
**独立テスト**: `modules/actor-core/tests/actor_system_scope/tests.rs` でスコープ内 spawn とメッセージ往復、`modules/actor-core/tests/actor_ref/tests.rs` でスコープ外利用の失敗を検証する。

### テスト

- [ ] T201 [P] [US1] `modules/actor-core/tests/actor_system_scope/tests.rs` にスコープ内 spawn の失敗テストを追加する
- [ ] T202 [P] [US1] `modules/actor-core/tests/actor_ref/tests.rs` にスコープ外 `ActorRef` 利用を拒否するテストを追加する

### 実装

- [ ] T203 [US1] `modules/actor-core/src/erased_message_envelope.rs` に `ErasedMessageEnvelope` を実装する
- [ ] T204 [US1] `modules/actor-core/src/message_adapter_registry.rs` に `MessageAdapterRegistry` を実装する
- [ ] T205 [US1] `modules/actor-core/src/actor_ref.rs` に `ActorRef<'scope, M>` を実装しライフタイム制約を付与する
- [ ] T206 [US1] `modules/actor-core/src/actor_context.rs` に `ActorContext<'scope, M>` を実装する
- [ ] T207 [US1] `modules/actor-core/src/actor_system_scope.rs` に `ActorSystemScope` を実装し状態管理と監査ログを追加する
- [ ] T208 [US1] `modules/actor-core/src/behavior_profile.rs` に `BehaviorProfile<M>` ビルダを実装する
- [ ] T209 [US1] 公開 API を整備し `modules/actor-core/src/lib.rs` で新規モジュールを再エクスポートする
- [ ] T210 [US1] Quickstart を更新し `specs/002-init-actor-lib/quickstart.md` にスコープ安全な利用例を記載する
- [ ] T211 [US1] スコープ生成エンドポイントを整合させ `specs/002-init-actor-lib/contracts/control-plane.yaml` を更新する

## フェーズ4: ユーザーストーリー2（優先度: P1） — メールボックスで負荷を制御したい

**目標**: Bounded/Unbounded Mailbox と Dispatcher 公平性を提供し、バックプレッシャーとイベントストリームを制御する。  
**独立テスト**: `modules/actor-core/tests/mailbox_runtime/tests.rs` で容量超過シナリオ、`modules/actor-core/tests/dispatcher/tests.rs` でラウンドロビン公平性を検証する。

### テスト

- [ ] T301 [P] [US2] `modules/actor-core/tests/mailbox_runtime/tests.rs` に容量超過で通知が発火するテストを追加する
- [ ] T302 [P] [US2] `modules/actor-core/tests/dispatcher/tests.rs` にラウンドロビン公平性のベンチマークテストを追加する

### 実装

- [ ] T303 [US2] `modules/actor-core/src/mailbox_runtime.rs` に `MailboxRuntime<M>` を実装し、CoreSync では `SyncQueue`、HostAsync では `AsyncQueue` をラップする `MailboxBackend` 抽象を確立する（`OverflowPolicy::Block` は後者のみ許可）。SystemMessageQueue と UserMessageQueue を内包し、Suspend/Resume 操作でユーザーキューのみを停止できるようにする
- [ ] T304 [US2] `modules/actor-core/src/dispatcher_runtime.rs` に `DispatcherRuntime` を実装し、`DispatcherConfig` と `FairnessStrategy` を利用してワーカー割当・スケジューリングを制御する
- [ ] T305 [US2] `modules/actor-core/src/message_invoker.rs` に `MessageInvoker<M>` を実装し、system/user 両キューからの取得順序と backpressure ヒント伝搬を担保する
- [ ] T306 [US2] バックプレッシャーメトリクスを `modules/actor-core/src/observation_channel.rs` に統合し、`OverflowPolicy::Block` 選択時は HostAsync キュー待機を含むヒントを発火する
- [ ] T307 [US2] `modules/actor-core/src/event_stream_core.rs` に `EventStreamCore` を実装し publish/backpressure を処理する
- [ ] T308 [US2] メールボックス設定エンドポイントを反映し `specs/002-init-actor-lib/contracts/control-plane.yaml` を更新する
- [ ] T309 [US2] Dispatcher 公平性の根拠を `specs/002-init-actor-lib/research.md` に追記する
- [ ] T310 [US2] Mailbox 設定例を `specs/002-init-actor-lib/quickstart.md` に追記する
- [ ] T311 [US2] Mailbox Middleware チェイン API を設計・実装し、メッセージ前後処理フックとテレメトリ統合を提供する
- [ ] T312 [US2] Throughput/Backpressure ヒントを ReadyQueueCoordinator に送出し DispatcherRuntime がワーカープール制御に利用できるよう統合する
- [ ] T313 [US2] Stash API と再投入制御ロジックを実装し、容量超過時の観測イベントとエラー伝搬をテストで保証する

## フェーズ5: ユーザーストーリー3（優先度: P1） — 失敗時の回復方針を制御したい

**目標**: `ActorError` と `SupervisionStrategy` により再起動・停止判定を制御し、観測チャンネルに結果を通知する。  
**独立テスト**: `modules/actor-core/tests/supervision/tests.rs` で Restart/Stop 分岐を検証し、致命的エラー時の停止とメトリクス記録を確認する。

### テスト

- [ ] T401 [P] [US3] `modules/actor-core/tests/supervision/tests.rs` に再起動回数と時間窓を検証するテストを追加する
- [ ] T402 [P] [US3] `modules/actor-core/tests/supervision/tests.rs` に致命的エラーで停止するテストを追加する

### 実装

- [ ] T403 [US3] `modules/actor-core/src/actor_error.rs` に `ActorError` と付随メタデータを実装する
- [ ] T404 [US3] `modules/actor-core/src/restart_statistics.rs` に `RestartStatistics` を実装する
- [ ] T405 [US3] `modules/actor-core/src/supervision_strategy.rs` に `SupervisionStrategy` と `SupervisionDecision` を実装する
- [ ] T406 [US3] Supervision 結果を適用するため `modules/actor-core/src/actor_system_scope.rs` を更新する
- [ ] T407 [US3] 監視プローブの契約を反映し `specs/002-init-actor-lib/contracts/control-plane.yaml` を更新する
- [ ] T408 [US3] Supervision 例を `specs/002-init-actor-lib/quickstart.md` に追記する

## フェーズ6: 仕上げ・横断対応

**目的**: ドキュメント整備と最終 CI を実施し、全ストーリーの成果を統合する。

- [ ] T501 [P] 実装メモをまとめ `specs/002-init-actor-lib/plan.md` に実施結果を追記する
- [ ] T502 研究ログを更新し `specs/002-init-actor-lib/research.md` に最終知見を記録する
- [ ] T503 `./scripts/ci-check.sh` を用いて最終 `./scripts/ci-check.sh all` を実行し結果を共有する
- [ ] T504 [P] `makers` ツールで `makers ci-check -- dylint` を実行しリンタ結果を共有する

---

## 依存関係と実行順序

1. フェーズ1（セットアップ）完了後にのみ他フェーズへ進む。  
2. フェーズ2（基盤整備）は全ストーリーの前提となる。`ObservationChannel` と ID 型が利用可能であることを確認してからストーリーを着手する。  
3. ユーザーストーリーは US1 → US2 → US3 の順に実装し、各ストーリー完了時点で独立テストを緑にする。  
4. フェーズ6（仕上げ）は全ストーリー完了後に横断的なドキュメント更新と最終 CI を行う。

### 依存グラフ

```
Setup (T001–T004)
  ↓
Foundation (T101–T106)
  ↓
US1 (T201–T211)
  ↓
US2 (T301–T309)
  ↓
US3 (T401–T408)
  ↓
Polish (T501–T504)
```

## 並列実行の例

- フェーズ1完了後、T102・T104・T106 は互いに依存しないため並列化可能。  
- US1 ではテスト作成（T201/T202）と `ErasedMessageEnvelope` 実装（T203）を並列に進められる。  
- US2 では T303（MailboxRuntime）と T306（EventStreamCore）を別担当で進め、合流時に T305 で観測統合を実施する。  
- US3 では T403（ActorError）と T404（RestartStatistics）を並行で実装し、T405 で統合する。

## 各ユーザーストーリーの独立テスト基準

- **US1**: `modules/actor-core/tests/actor_system_scope/tests.rs` と `modules/actor-core/tests/actor_ref/tests.rs` が緑で、`ActorRef` をスコープ外にムーブすると明示的なエラーとなる。  
- **US2**: `modules/actor-core/tests/mailbox_runtime/tests.rs` が容量超過時の通知を検証し、`modules/actor-core/tests/dispatcher/tests.rs` が公平性メトリクスを検証する。  
- **US3**: `modules/actor-core/tests/supervision/tests.rs` で再起動上限と致命的停止が期待通りに動作する。

## 実装戦略（MVP → 拡張）

1. **MVP (US1)**: スコープ安全な ActorSystem と基本 Behavior API を実装し、Quickstart を用いてメッセージ往復を実証する。  
2. **拡張1 (US2)**: Mailbox/Dispatcher/EventStream を追加し、バックプレッシャー制御と公平性メトリクスを実現する。  
3. **拡張2 (US3)**: Supervision と ActorError 分類を導入し、エラー復旧ポリシーを制御する。  
4. **仕上げ**: ドキュメントを更新し、CI/リンタを完走させて安定版を確定する。
