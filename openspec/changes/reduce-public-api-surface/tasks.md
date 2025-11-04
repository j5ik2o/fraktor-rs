# タスク: actor-coreの公開API表面積削減

## Phase 1: 高優先度メソッド（21個）

### SystemStateGeneric の内部実装化 (6個)

- [ ] `register_cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `remove_cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `cell`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `send_system_message`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `notify_failure`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `mark_terminated`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### DispatcherGeneric の内部実装化 (7個)

- [ ] `register_invoker`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `enqueue_user`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `enqueue_system`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `schedule`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `mailbox`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `create_waker`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `into_sender`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/dispatcher/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### MailboxGeneric の内部実装化 (7個)

- [ ] `set_instrumentation`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `enqueue_system`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `enqueue_user`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `enqueue_user_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `poll_user_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `dequeue`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `suspend`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `resume`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 1 統合テスト

- [ ] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [ ] actor-stdのテスト実行（影響確認）
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [ ] examples実行確認
  - コマンド: `cargo run --example ping_pong_no_std`
  - コマンド: `cargo run --example deadletter`

- [ ] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## Phase 2: 中優先度メソッド（8個）

### SystemStateGeneric の名前・子管理メソッド (8個)

- [ ] `assign_name`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `release_name`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `set_user_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `clear_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `user_guardian`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `register_child`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `unregister_child`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `child_pids`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 2 統合テスト

- [ ] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [ ] actor-stdのテスト実行
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [ ] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## Phase 3: 低優先度メソッド（7個）

### SystemStateGeneric の Future/エラー管理メソッド (4個)

- [ ] `register_ask_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `drain_ready_ask_futures`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `record_send_error`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `termination_future`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/system/system_state.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### MailboxGeneric のテスト用メソッド (3個)

- [ ] `is_suspended`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `user_len`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

- [ ] `system_len`を`pub(crate)`に変更
  - ファイル: modules/actor-core/src/mailbox/base.rs
  - 検証: `cargo test -p cellactor-actor-core-rs --lib`

### Phase 3 統合テスト

- [ ] actor-core全体のテスト実行
  - コマンド: `cargo test -p cellactor-actor-core-rs`

- [ ] actor-stdのテスト実行
  - コマンド: `cargo test -p cellactor-actor-std-rs`

- [ ] CI全体実行
  - コマンド: `./scripts/ci-check.sh all`

## 最終確認

### ドキュメント更新

- [ ] CHANGELOG.mdに破壊的変更を記載
  - セクション: `## [Unreleased] - BREAKING CHANGES`
  - 内容: 36個のメソッドが`pub(crate)`化されたことを記載

- [ ] MIGRATION.mdを作成または更新
  - 内容: 移行ガイドとactor-std経由での使用方法

- [ ] cargo docでドキュメント生成確認
  - コマンド: `cargo +nightly doc --no-deps -p cellactor-actor-core-rs`
  - 確認: 内部実装メソッドが公開ドキュメントから消えていること

### 総合テスト

- [ ] 全パッケージのテスト実行
  - コマンド: `cargo test --workspace`

- [ ] 全examplesの実行確認
  - コマンド: `./scripts/run-examples.sh` （存在する場合）
  - または個別に: `cargo run --example <name>` for all examples

- [ ] CI完全実行
  - コマンド: `./scripts/ci-check.sh all`
  - 期待: 全チェックがパス

### レビュー

- [ ] 変更箇所のコードレビュー
  - 全36箇所の変更を確認
  - `pub fn` → `pub(crate) fn`への変更が正しいことを確認

- [ ] APIドキュメントレビュー
  - 公開APIが明確になっていることを確認
  - 内部実装が隠蔽されていることを確認

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
