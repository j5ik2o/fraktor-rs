## 1. 契約文書化

- [x] 1.1 `EventStreamShared::subscribe_with_key` の rustdoc に replay は return 前に同期通知されるが concurrent publish との厳密順序は未保証であることを追記する。
- [x] 1.2 `EventStreamShared::publish` の rustdoc に callback 完了の同期観測契約と panic 伝播契約を追記する。
- [x] 1.3 `EventStreamSubscriberShared::notify` の rustdoc に callback panic を catch せず subscription lifecycle を変更しないことを追記する。

## 2. 回帰テスト

- [x] 2.1 `subscribe_with_key` return 後に replay が観測済みであることを確認する test を追加する。
- [x] 2.2 `publish` return 後に subscriber callback が観測済みであることを確認する test を追加する。
- [x] 2.3 `publish` 中の subscriber panic が呼び出し元へ伝播し、subscription が自動解除されないことを確認する test を追加する。
- [x] 2.4 `publish` callback 中の unsubscribe が進行中 publish の配送 snapshot を変更しないことを確認する test を追加する。
- [x] 2.5 `publish` callback 中の subscribe が進行中 publish の配送 snapshot を変更しないことを確認する test を追加する。

## 3. 検証

- [x] 3.1 actor-core event stream の targeted tests を実行する。
- [x] 3.2 `mise exec -- openspec validate define-eventstream-replay-publish-panic-contract --strict` を実行する。
- [x] 3.3 `cargo fmt --check` または `cargo fmt` と `git diff --check` を実行する。
