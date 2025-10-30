1. [x] `SyncPriorityBackend` の実装を `T: PriorityMessage` 前提に更新し、`PriorityMessage::get_priority()` を反映した優先度順序を実現する。
2. [x] BinaryHeap ベースの実装を維持しつつ、優先度値と FIFO 順序を両立させる設計を導入する。
3. [x] 優先度値（`Some`/`None`）の差異をカバーするテストを `modules/utils-core/src/collections/queue/backend/sync_priority_backend/tests.rs` に追加・更新する。
4. [x] `cargo fmt` と対象モジュールのテスト（例: `cargo test -p cellactor-utils-core-rs sync_priority_backend`) を実行して正常終了を確認する。
5. [ ] `./scripts/ci-check.sh all` を実行し、変更によるリグレッションがないことを確認する。
6. [ ] すべてのチェック項目の完了をレビューし、`tasks.md` のステータスを更新する。
