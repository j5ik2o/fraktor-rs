# タスク: actor-coreの公開API表面積削減

## Phase 1: 高優先度メソッド（21個）

### SystemStateGeneric の内部実装化 (6個)

- [x] `register_cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `remove_cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `send_system_message`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `notify_failure`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `mark_terminated`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### DispatcherGeneric の内部実装化 (7個)

- [x] `register_invoker`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `enqueue_user`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `enqueue_system`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `schedule`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `mailbox`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `create_waker`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `into_sender`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### MailboxGeneric の内部実装化 (7個)

- [x] `set_instrumentation`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `enqueue_system`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `enqueue_user`をテストアクセス可能な`pub`に変更（`#[doc(hidden)]`付き）
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`
  - 注記: 統合テストからのアクセスのため`pub`のまま、`#[doc(hidden)]`でドキュメントから隠蔽

- [x] `enqueue_user_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `poll_user_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `dequeue`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `suspend`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `resume`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 1 統合テスト

- [x] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [x] actor-stdのテスト実行（影響確認）
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [x] examples実行確認
  - コマンド: `cargo run --example ping_pong_no_std`
  - コマンド: `cargo run --example deadletter`

- [x] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## Phase 2: 中優先度メソッド（8個）

### SystemStateGeneric の名前・子管理メソッド (8個)

- [x] `assign_name`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `release_name`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `set_user_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `clear_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `user_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `register_child`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `unregister_child`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `child_pids`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 2 統合テスト

- [x] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [x] actor-stdのテスト実行
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [x] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## Phase 3: 低優先度メソッド（7個）

### SystemStateGeneric の Future/エラー管理メソッド (4個)

- [x] `register_ask_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `drain_ready_ask_futures`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `record_send_error`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `termination_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### MailboxGeneric のテスト用メソッド (3個)

- [x] `is_suspended`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `user_len`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [x] `system_len`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 3 統合テスト

- [x] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [x] actor-stdのテスト実行
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [x] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## 最終確認

### ドキュメント更新

- [ ] CHANGELOG.mdに破壊的変更を記載
  - セクション: `## [Unreleased] - BREAKING CHANGES`
  - 内容: 35個のメソッドが`pub(crate)`化、1個がテストアクセス用`pub + #[doc(hidden)]`化されたことを記載

- [ ] MIGRATION.mdを作成または更新
  - 内容: 移行ガイドとactor-std経由での使用方法

- [ ] cargo docでドキュメント生成確認
  - コマンド: `cargo +nightly doc --no-deps -p cellactor-actor-core-rs`
  - 確認: 内部実装メソッドが公開ドキュメントから消えていること

### 総合テスト

- [x] 全パッケージのテスト実行
  - コマンド: `cargo test --workspace`
  - 結果: 185テストが成功

- [x] 全examplesの実行確認
  - コマンド: `./scripts/ci-check.sh all` で実行
  - 結果: ping_pong_no_std, deadletter, logger_subscriber, named_actor, ping_pong_tokio, supervision すべて成功

- [x] CI完全実行
  - コマンド: `./scripts/ci-check.sh all`
  - 期待: 全チェックがパス
  - 結果: 全チェック成功（clippy, dylint, tests, examples）

### レビュー

- [x] 変更箇所のコードレビュー
  - 全36箇所の変更を確認
  - 35個が`pub fn` → `pub(crate) fn`、1個が`pub fn`のまま`#[doc(hidden)]`付与
  - `#[allow(dead_code)]`と`#[allow(clippy::wrong_self_convention)]`を適切に追加

- [x] APIドキュメントレビュー
  - 公開APIが明確になっていることを確認
  - 内部実装が隠蔽されていることを確認（`#[doc(hidden)]`含む）

## 完了条件

全てのタスクが✅になり、以下の条件を満たすこと:

1. 36個のメソッドが`pub(crate)`化されている
2. 全テストがパスする
3. `./scripts/ci-check.sh all`が成功する
4. examplesが正常に動作する
5. ドキュメントが更新されている

## ロールバック手順

問題が発生した場合:

1. 該当Phaseの変更をrevert
2. テスト実行で問題箇所を特定
3. 必要に応じて個別メソッドを`pub`に戻す
4. issue作成して対応方針を検討
