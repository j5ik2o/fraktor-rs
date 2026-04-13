## 1. user_queue_lock → put_lock 改名

- [ ] 1.1 `Mailbox` struct の `user_queue_lock` フィールドを `put_lock` に改名する
- [ ] 1.2 `MailboxSharedSet` の `user_queue_lock` を `put_lock` に改名する
- [ ] 1.3 全参照箇所のフィールド名を更新する

## 2. 通常 enqueue/dequeue から lock 除去

- [ ] 2.1 `enqueue_envelope_locked` から `put_lock` の取得を除去する（`is_closed()` チェック + `user.enqueue()` を直接呼び出し）
- [ ] 2.2 `dequeue` から `put_lock` の取得を除去する（`user.dequeue()` を直接呼び出し）
- [ ] 2.3 `user_len` から `put_lock` の取得を除去する（`user.number_of_messages()` を直接呼び出し）
- [ ] 2.4 `publish_metrics` から `put_lock` の取得を除去する

## 3. compound op の lock 維持を確認

- [ ] 3.1 `prepend_user_messages_deque_locked` が `put_lock` を取得していることを確認する
- [ ] 3.2 `become_closed_and_clean_up` が `put_lock` を取得していることを確認する
- [ ] 3.3 `put_lock` のドキュメントに「compound op のみで取得」を明記する

## 4. 検証

- [ ] 4.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [ ] 4.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [ ] 4.3 `./scripts/ci-check.sh` が全パスすることを確認する
- [ ] 4.4 `cargo bench --features tokio-executor -p fraktor-actor-adaptor-std-rs` で before/after を比較する（Bounded 3→2 段、Unbounded 2→1 段の効果を計測）
