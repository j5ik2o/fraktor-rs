## 1. user_queue_lock → put_lock 改名

- [x] 1.1 `Mailbox` struct の `user_queue_lock` フィールドを `put_lock` に改名する
- [x] 1.2 `MailboxSharedSet` の `user_queue_lock` を `put_lock` に改名する
- [x] 1.3 全参照箇所のフィールド名を更新する（base.rs, tests.rs, mailbox_shared_set.rs）

## 2. dequeue / metrics から lock 除去

- [x] 2.1 `dequeue` から `put_lock` の取得を除去する（`is_close_requested()` + `user.dequeue()` を直接呼び出し）
- [x] 2.2 `user_len` から `put_lock` の取得を除去する（`user.number_of_messages()` を直接呼び出し）
- [x] 2.3 `publish_metrics` から `put_lock` の取得を除去する

## 3. compound op の lock 維持を確認

- [x] 3.1 `enqueue_envelope_locked` が `put_lock` を取得していることを確認する（TOCTOU race 防止のため維持必須）
- [x] 3.2 `prepend_user_messages_deque_locked` が `put_lock` を取得していることを確認する
- [x] 3.3 `finalize_cleanup` が `put_lock` を取得していることを確認する
- [x] 3.4 `put_lock` のドキュメントに「compound op のみで取得」を明記する

## 4. 検証

- [x] 4.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [x] 4.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [x] 4.3 `./scripts/ci-check.sh` が全パスすることを確認する（146 tests passed）
- [x] 4.4 `cargo bench --features tokio-executor -p fraktor-actor-adaptor-std-rs` で before/after を比較する
  - `bounded_capacity_1`: **-5.7%** (231.60ns, p=0.00) Performance improved
  - `bounded_capacity_64`: 変化なし (p=0.58)
  - `default_dispatcher_baseline`: **-8.9%** (p=0.00) Performance improved
  - enqueue ベンチマーク: 変化なし（lock を維持したため期待通り）
