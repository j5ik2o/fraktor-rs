---

description: "セルアクター no_std ランタイム初期版の実装タスクリスト"

---

# タスク: セルアクター no_std ランタイム初版（再設計）

**入力**: `/specs/001-add-actor-runtime/` 配下の設計ドキュメント
**前提条件**:
- **`/modules/actor-core-old`, `/modules/actor-std-old`のコードを重視してください**。
- plan.md（必須）、spec.md（ユーザーストーリー参照）、research.md、data-model.md、contracts/
**テスト方針**: 原則2に従い、ユーザーストーリー単位で独立した検証ができるようにする。`modules/actor-core/tests/` にストーリー別の統合テストを追加し、`cfg(test)` 下でのみ `std` を有効化する。実装前に既存コードの設計パターン（1ファイル1構造体／trait、`ArcShared` 抽象、`no_std` 運用）を確認し、乖離する場合は理由と影響を記録する。共有参照・ロックは必ず `modules/utils-core` の抽象 (`Shared`/`ArcShared`, `Async/SyncMutexLike`) を利用し、`alloc::sync::Arc` へ直接依存しない。API とデータフローは借用ベースのライフタイム設計を採り、ヒープ確保は不可避な箇所に限定して計測・再利用戦略をタスク内で明示する。`sender()` は導入せず、メッセージの `reply_to: ActorRef` を必須パターンとする。`./scripts/ci-check.sh all` と `makers ci-check -- dylint` は全タスク完了時に一括で実行し、それ以前は対象範囲のテストとローカル検証を優先する。
**進行中指針**: cargo checkをしながら作業すること。単体テストを書くこと。
**コーディング規約**: `vec!` マクロ使用時は `use alloc::vec;` を追加。コンパイル時評価可能な関数は `const fn` を使用。値渡しよりも参照渡し（`&T`）を優先してクローンを回避。すべての公開関数に適切な rustdoc コメント（`# Errors`, `# Panics` セクション含む）を記載。Clippy 警告は原則すべて修正するが、借用チェッカーとの兼ね合いで適用不可能な場合は `#[allow(...)]` で理由をコメント付きで許可し、`plan.md` の「複雑度トラッキング」に記録。
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

- [x] T001 ワークスペースのマニフェストを更新し、`modules/actor-core` のフィーチャとデフォルト有効化フラグを公開する (Cargo.toml)
- [x] T002 `modules/actor-core/Cargo.toml` の依存関係を調整し、`no_std + alloc` 対応のために `portable-atomic`・`heapless`・`portable-atomic-util`・`modules/utils-core` を正しく設定する (modules/actor-core/Cargo.toml)
- [x] T003 クレートルートに `#![no_std]`・モジュール宣言・共通再エクスポートを整備する (modules/actor-core/src/lib.rs)
- [x] T004 CI パイプラインに `cargo check --no-default-features --package actor-core` を組み込み、scripts から実行できるようにする (scripts/ci-check.sh)

---

## フェーズ2: 基盤整備（全ストーリーに必須）

**目的**: すべてのストーリーで共有するコア抽象（Actor/Context/Error/Message など）を定義する。

- [x] T005 `Actor` トレイトを定義し、`pre_start` / `receive` / `post_stop` のライフサイクルシグネチャを揃える (modules/actor-core/src/actor.rs)
- [x] T006 `ActorContext` の骨組みを実装し、self PID・子生成フック・返信ヘルパーを提供する (modules/actor-core/src/actor_context.rs)
- [x] T007 `Recoverable` / `Fatal` 変種を備えた `ActorError` 列挙体と補助コンストラクタを追加する (modules/actor-core/src/actor_error.rs)
- [x] T008 型 ID メタデータとダウンキャストユーティリティを備えた `AnyMessage` ラッパーを実装する (modules/actor-core/src/any_message.rs)
- [x] T009 ポーリングベースの完了コールバックを持つ `ActorFuture` の骨格を用意する (modules/actor-core/src/actor_future.rs)
- [x] T010 `Pid` 構造体と O(1) で引けるレジストリキーを定義する (modules/actor-core/src/pid.rs)
- [x] T011 親スコープ内で一意な名前と自動 `anon-{pid}` 生成を行う `NameRegistry` を実装する (modules/actor-core/src/name_registry.rs)
- [x] T012 become/unbecome スタックを扱う `ReceiveState` 状態機械を作成する (modules/actor-core/src/receive_state.rs)
- [x] T013 `SupervisorStrategy`（OneForOne / AllForOne / decider）のデータ構造を定義する (modules/actor-core/src/supervisor_strategy.rs)
- [x] T014 `Props` ビルダーと `MailboxConfig`・`SupervisorOptions` の定義を追加する (modules/actor-core/src/props.rs)
- [x] T015 DropNewest / DropOldest / Grow / Block と Bounded / Unbounded フラグを網羅する `MailboxPolicy` 列挙体を定義する (modules/actor-core/src/mailbox_policy.rs)

---

## フェーズ3: ユーザーストーリー 1（優先度: P1） 🎯 MVP

**目標**: AnyMessage を使った最小構成でアクターを起動し、Ping/Pong サンプルが no_std + alloc 環境で動作する。さらに DispatcherConfig を介して std + Tokio ランタイム上でも同サンプルが完走することを確認する。
**独立テスト**: `modules/actor-core/tests/ping_pong.rs` で spawn / tell / 背圧ポリシー / reply_to 処理が通ること。

- [x] T016 [US1] `ActorRef` ハンドルを実装し、未型付けの `tell` / `ask` API と ArcShared ストレージを備える（所有型 `AnyMessage` を受け付け、送信失敗を `Result` で検知可能にする）(modules/actor-core/src/actor_ref.rs)
- [x] T017 [US1] DropNewest / DropOldest / Grow ポリシーと Bounded / Unbounded 容量を扱う `Mailbox` を SyncQueue バックエンドで実装する (modules/actor-core/src/mailbox.rs)
- [x] T018 [US1] スループット制限とスケジューリングフックを備えた `Dispatcher` を実装する (modules/actor-core/src/dispatcher.rs)
- [x] T019 [US1] ミドルウェアチェーンと `reply_to` ルーティングを行う `MessageInvoker` パイプラインを実装する (modules/actor-core/src/message_invoker.rs)
- [x] T020 [US1] ガーディアン Props、`user_guardian_ref()`、名前レジストリ、`spawn_child` を通じた生成、`reply_to` ディスパッチ、`ActorCell` 管理を含む `ActorSystem` コアを実装する (modules/actor-core/src/system.rs, modules/actor-core/src/actor_cell.rs)
- [x] T021 [US1] `ActorFuture` の ask ヘルパーを完成させ ActorSystem と連携させる (modules/actor-core/src/actor_future.rs)
- [x] T022 [P] [US1] AnyMessage + reply_to を用いた no_std Ping/Pong サンプルを追加する (modules/actor-core/examples/ping_pong_no_std/main.rs; `ctx.self_ref()` を payload の `reply_to` に埋め込み、`reply_to.tell(...)` で応答する例を示す。実行は `cargo run -p fraktor-actor-core-rs --example ping_pong_no_std --features std`)
- [x] T022A [P] [US1] Tokio ランタイムの `Handle::spawn_blocking` を用いて Dispatcher を駆動する `TokioExecutor` を examples 配下に追加し、`cfg(feature = "std")` 下でのみコンパイルされるようにする (modules/actor-core/examples/ping_pong_tokio/executor.rs)
- [x] T022B [P] [US1] `Props::with_dispatcher(DispatcherConfig::from_executor(...))` を利用する Ping/Pong サンプルを追加し、Tokio ランタイムで ActorSystem を起動して `reply_to` ベースの応答とスレッド ID ログを検証しつつ、`when_terminated()` の Future/Listener でシステム終了を待機する (modules/actor-core/examples/ping_pong_tokio/main.rs; 実行コマンド `cargo run -p fraktor-actor-core-rs --example ping_pong_tokio --features std`)
- [ ] T022C [Optional] DispatcherConfig / Props の利便性向上ヘルパー（例: `DispatcherConfig::tokio_current()` や `Props::with_tokio_dispatcher()`）の設計案をまとめ、導入時の API 影響とボイラープレート削減効果を評価する (docs/ 或いは research.md にメモ)
- [x] T023 [P] [US1] spawn / tell / 背圧ポリシー / 自動命名を検証する統合テストを追加する (modules/actor-core/tests/ping_pong.rs)

## フェーズ2.5: ツールボックス抽象導入

**目的**: `SyncMutexFamily` / `RuntimeToolbox` を基盤に据え、ランタイムの同期プリミティブ差し替えを仕様どおり実現する。

- [x] T020D [US1] `modules/utils-core` に `sync/mutex_family.rs` と `sync/runtime_toolbox.rs` を追加し、`SyncMutexFamily`・`RuntimeToolbox`・`NoStdToolbox` を実装する。`SpinMutexFamily` の単体テストを用意し、FR-036/FR-037 を満たす (modules/utils-core/src/sync/).
- [x] T020E [US1] `modules/actor-core` 全体を `ToolboxMutex<T, TB>` ベースへリファクタリングし、`ActorSystemGeneric<TB>` / `ActorCell<TB>` / `Mailbox<TB>` / `EventStreamGeneric<TB>` / `DeadletterGeneric<TB>` / `ActorFutureGeneric<TB>` を導入する。公開 API は `type ActorSystem = ActorSystemGeneric<NoStdToolbox>` など型エイリアスで互換性を維持し、FR-038 を満たす。
- [x] T020F [US1] `modules/utils-std` に `StdMutexFamily` と `StdToolbox` を実装し、`modules/actor-std` から再エクスポートして例示コード・統合テストから `StdActorSystem` を利用できるようにする。Tokio サンプルで `StdToolbox` を選択し、FR-037/FR-039 を検証する。
- [x] T020G [US1] ビルダー／ドキュメントを更新し、`Props<StdToolbox>` を用いた切り替え手順と `StdActorSystem` の利用方法、CI での `cargo check --features std` 実行方針を quickstart・docs に反映する。FR-039/FR-040 の受け入れ条件を満たす。

---

## フェーズ4: ユーザーストーリー 2（優先度: P2）

**目標**: 親子アクターの監督ツリーを構築し、Supervisor 戦略に基づく再起動／停止を実現する。
**独立テスト**: `modules/actor-core/tests/supervisor.rs` で Restart/Escalate ポリシーと子アクター監視が検証できること。

- [x] T024 [US2] レート制限付き再起動を追跡する `RestartStatistics` を実装する (modules/actor-core/src/restart_statistics.rs)
- [x] T025 [US2] `SupervisorStrategy` の判定ロジックを配線し Restart/Fatal/Escalate を処理する (modules/actor-core/src/supervisor_strategy.rs)
- [x] T026 [US2] `ActorContext` を拡張し、`spawn_child`・子レジストリ・スーパービジョンシグナルを提供する (modules/actor-core/src/actor_context.rs)
- [x] T027 [US2] `ActorSystem` とスーパービジョンツリーの連携を実装し、障害を親へ伝播させる (modules/actor-core/src/system.rs)
- [x] T027A [US2] `ActorSystem::terminate()` / `when_terminated()` / `run_until_terminated()` を実装し、ガーディアン停止とシステム終了待機を整備する (modules/actor-core/src/system.rs, modules/actor-core/src/system_state.rs, modules/actor-core/tests/system_lifecycle.rs, specs/001-add-actor-runtime/quickstart.md)
- [x] T027B [US2] `ctx.stop_self()` / `SystemMessage::Stop` による停止が子アクターへ伝播するよう、ActorCell / ActorSystemState に子停止伝播処理を追加し、挙動をドキュメント化・テストで検証する (modules/actor-core/src/actor_cell.rs, modules/actor-core/src/system_state.rs, modules/actor-core/src/system/tests.rs, specs/001-add-actor-runtime/quickstart.md, specs/001-add-actor-runtime/spec.md)
- [x] T028 [US2] 子アクターを扱う `ChildRef` ラッパーを追加しライフサイクルフックを提供する (modules/actor-core/src/child_ref.rs)
- [x] T029 [P] [US2] Restart/Escalate / panic 非介入をカバーするスーパービジョン回帰テストを追加する (modules/actor-core/tests/supervisor.rs)

---

## フェーズ5: ユーザーストーリー 3（優先度: P3）

**目標**: EventStream / Deadletter / Logger によるオブザーバビリティとホスト制御面を提供する。
**独立テスト**: `modules/actor-core/tests/event_stream.rs` で LogEvent 配信・Deadletter 記録・容量警告が検証できること。

- [x] T030 [US3] バッファ付き配信を行う `EventStream` の publish/subscribe バスを実装する (modules/actor-core/src/event_stream.rs)
- [x]  T031 [US3] EventStream へ転送する `Deadletter` ストアを実装する (modules/actor-core/src/deadletter.rs)
- [x]  T032 [US3] LogEvent を UART/RTT へルーティングする `LoggerSubscriber` を実装する (modules/actor-core/src/logger_subscriber.rs)
- [x] T033 [US3] ActorSystem / Supervisor 経路からライフサイクル・ログイベントを発火させる (modules/actor-core/src/system.rs)
- [x] T034 [US3] Mailbox に容量警告とスループットメトリクスを組み込む (modules/actor-core/src/mailbox.rs)
- [x] T035 [US3] MessageInvoker の middleware / pipeline 実装を actor-old から移植する (modules/actor-core/src/message_invoker.rs, modules/actor-core-old/src/message_invoker/)
- [x] T036 [P] [US3] EventStream + Deadletter フローを検証する統合テストを追加する (modules/actor-core/tests/event_stream.rs)
- [x] T037 [P] [US3] LogEvent を消費するロガー購読者サンプルを追加する (modules/actor-std/examples/logger_subscriber_std/main.rs)
- [x] T037B [P] [US3] Deadletter 監視とサスペンド郵便受けを示すサンプルを追加する (modules/actor-std/examples/deadletter_std/main.rs)
- [x] T037A [Optional] EventStream/Deadletter のバッファ容量と警告閾値をユーザ設定できる API を検討し、quickstart/data-model に推奨値を追記する。Tokio などホスト側ランタイム向けの `DispatcherConfig` ヘルパーは core ではなく `actor-std` 等の拡張クレートで提供する方針案をまとめる。将来的に `actor-std` クレートへヘルパー API を追加する際は、quickstart の該当節へ反映済みかを必ず確認する。

---

## フェーズ6: 仕上げ・横断対応

**目的**: ドキュメント整備・性能検証・最終 CI を実施する。

- [ ] T038 ランタイムガイドを更新し、利用方法・reply_to パターン・監視手順を追記する (docs/guides/actor-system.md)
- [ ] T039 Mailbox / Dispatcher のスループットベンチマークハーネスを追加する (modules/actor-core/tests/perf_mailbox.rs)
- [ ] T040 Makefile のレシピを更新し、actor-core のストーリーパイプラインと最終 CI ターゲットを含める (Makefile.toml)
- [ ] T041 [Optional] `actor-std` クレート向けに `ActorSystemConfig`（仮称）を設計し、EventStream/Deadletter 容量や警告閾値を指定できるヘルパー API の草案と quickstart 反映手順を整理する（設計メモを research.md か docs/ に追記し、導入時の変更点を quickstart アップデートとセットで管理する）。
- [ ] T042 `actor-std` クレートに DispatcherConfig / Props の Tokio ヘルパー（例: `DispatcherConfig::tokio_current()`, `Props::with_tokio_dispatcher()`）を実装し、対応する quickstart/plan/specs の記述を更新する。Tokio ランタイムの `Handle` を安全に取得できる API 設計と、`actor-core` 側の no_std ポリシーを崩さない構成を確認する。

---

## 依存関係と実行順序

- フェーズ1 → フェーズ2 → US1 → US2 → US3 → フェーズ6
- US1 完了が US2 / US3 の前提。US2 と US3 はそれぞれ独立テストが通ったあとフェーズ6へ進む。

## 並列実行の例

- US1: T022 / T022A / T022B / T023 は T020 完了後に並列実行可。
- US2: T024・T025 完了後に T029 を並列で進められる。
- US3: T036 と T037 は T033 まで完了していれば同時着手可。
- フェーズ6: T038 と T039 は実装完了後に並列実行し、最後に T040 で仕上げ。

## 実装戦略

1. **MVP (US1)**: ActorSystem、ActorRef、Mailbox、Dispatcher、MessageInvoker を最小構成で完成させ、no_std 向け Ping/Pong サンプルと統合テストを通す。あわせて DispatcherConfig 経由で TokioExecutor を差し替えた std 向け Ping/Pong サンプルで Dispatcher 拡張性を実証する。
2. **信頼性 (US2)**: RestartStatistics・SupervisorStrategy・子アクター監視を追加し、panic 非介入ポリシーとイベント通知を確立する。
3. **オブザーバビリティ (US3)**: EventStream/Deadletter/Logger を導入し、OpenAPI ベースのホスト制御面を提供する。
4. **Polish**: ドキュメント／ベンチマーク／CI を整え、no_std + alloc での運用を確実にする。
