## 1. prepend caller inventory の固定

- [ ] 1.1 `Mailbox::prepend_user_messages(...)` の current caller を棚卸しし、production caller と test-only caller を分離する
- [ ] 1.2 `ActorCell::unstash_*` が唯一の production caller であることを確認し、必要なら設計書に反映する
- [ ] 1.3 `./scripts/ci-check.sh ai dylint` を実行し、Section 1 の変更で lint エラーが増えていないことを確認する

## 2. test fixture の先行移行

- [ ] 2.1 `base/tests.rs` の prepend close-race / block テストが deque queue 前提で維持できるように、必要な deque-capable fixture を設計する
- [ ] 2.2 `ScriptedDequeMessageQueue` 等の deque-capable test fixture が必要なら追加する
- [ ] 2.3 旧 fallback 前提の prepend テストを、新 fixture と新 contract に合わせて先に移行する
- [ ] 2.4 `./scripts/ci-check.sh ai dylint` を実行し、Section 2 の変更で lint エラーが増えていないことを確認する

## 3. prepend contract の hardening

- [ ] 3.1 mailbox から deque capability を caller が事前解決するための crate-private API を追加する
- [ ] 3.2 deque 専用 prepend API を `Mailbox` の method として追加し、lock 責務は `Mailbox` 側に残したまま empty batch は引き続き `Ok(())` を返す
- [ ] 3.3 `ActorCell::unstash_*` を新しい deque 専用 prepend API へ移行し、`user_queue_is_deque_capable()` 呼び出しは `user_deque()` の `Option` 解決に置き換える。必要なら helper 自体の存続/撤去を決める
- [ ] 3.4 `prepend_via_drain_and_requeue(...)` と関連する recovery path を削除する
- [ ] 3.5 `prepend_would_overflow` が prepend path で不要になるなら撤去し、残すなら理由をコメントまたは test で固定する。`prepend_via_deque` は新 API 本体へ統合または流用する
- [ ] 3.6 `./scripts/ci-check.sh ai dylint` を実行し、Section 3 の変更で lint エラーが増えていないことを確認する

## 4. contract 固定テストの追加・更新

- [ ] 4.1 non-deque mailbox では caller が prepend 前に失敗することを test で固定する
- [ ] 4.2 empty batch に対する prepend が no-op のまま維持される test を追加する
- [ ] 4.3 deque mailbox に対する prepend ordering が維持される test を維持または補強する
- [ ] 4.4 `base/tests.rs` の prepend close-race / block テストが新 contract 下でも維持されることを確認する
- [ ] 4.5 `ActorCell::unstash_*` が fallback ではなく deque path または early failure に限定されることを test で固定する
- [ ] 4.6 stash / persistence / showcase caller が prepend contract を満たしていることを確認し、必要な test を更新する
- [ ] 4.7 `./scripts/ci-check.sh ai dylint` を実行し、Section 4 の変更で lint エラーが増えていないことを確認する

## 5. 検証

- [ ] 5.1 `cargo check -p fraktor-actor-core-rs --lib` clean
- [ ] 5.2 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass
- [ ] 5.3 `cargo test -p fraktor-persistence-core-rs --lib` 全件 pass
- [ ] 5.4 `cargo check -p fraktor-showcases-std --examples` clean
- [ ] 5.5 `openspec validate mailbox-prepend-requires-deque --strict` valid
- [ ] 5.6 TAKT ピース実行時は中間では `./scripts/ci-check.sh ai dylint` のみを使い、最終検証として `./scripts/ci-check.sh ai all` を 1 回だけ実行する
- [ ] 5.7 最終的に `./scripts/ci-check.sh ai all` exit 0
