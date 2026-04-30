# stream-island-actors 実装計画 3.6 / 3.7 / 7.3

## 対象

- 3.6: `ActorMaterializer::new_without_system` 相当の公開 helper を削除するか、`#[cfg(test)] pub(crate)` のテスト専用 API に縮小する。
- 3.7: ActorSystem なしで `start()` / `materialize()` が成功する経路が残っていないことを test または compile check で固定する。
- 7.3: ActorSystem なしの直実行 API や `collect_values()` 相当 helper を公開 API に戻さない。

## 実装方針

- `ActorMaterializer::new_without_system` は既存の unit test だけが使うため、`#[cfg(test)] pub(crate)` に縮小する。
- write_tests ステップで追加済みの `materialize_fails_without_actor_system` と `public_api_guard` を実装完了の検証として使う。
- `collect_values()` 相当の helper は追加しない。既存の private helper は公開 API ではないため触れない。
- バッチ外の lifecycle / cancellation / rustdoc / showcase タスクには触れない。

## 検証

- `rtk rustup run nightly-2025-12-01 cargo fmt --all --check`
- `rtk cargo test -p fraktor-stream-core-rs actor_materializer`
- `rtk cargo test -p fraktor-stream-core-rs public_stream_api_does_not_expose_actor_systemless_helpers`
- `rtk git diff --check`
