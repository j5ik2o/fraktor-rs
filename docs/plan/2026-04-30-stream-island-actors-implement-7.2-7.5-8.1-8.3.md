# stream-island-actors 実装計画 7.2 / 7.5 / 8.1 / 8.2 / 8.3

## 対象変更

- change: `stream-island-actors`
- tasks_path: `openspec/changes/stream-island-actors/tasks.md`

## 今回のバッチ

| タスクID | 内容 |
|----------|------|
| 7.2 | stream showcase は ActorSystem + ActorMaterializer + Sink 経由の実行だけを示すように維持する |
| 7.5 | カスタム stream mailbox selector は本 change に含めず、必要なら別 change として整理する |
| 8.1 | fast feedback として `rtk cargo test -p fraktor-stream-core-rs` を実行する |
| 8.2 | 必要に応じて `rtk cargo test -p fraktor-showcases-std --features advanced` を実行する |
| 8.3 | `rtk git diff --check` を実行する |

## 実装方針

- stream showcase が `support::start_materializer()`、`Sink`、`graph.run(&mut materializer)` 経由の実行だけを示していることを確認する。
- ActorSystem なし直実行 helper、`collect_values()` 相当 helper、`ActorMaterializer::new_without_system` を公開 API に戻さない。
- カスタム stream mailbox selector API / 設定 / 公開型は追加しない。
- 7.2 / 7.5 が既存実装と追加済みガードテストで満たされている場合、プロダクションコードは変更しない。
- 検証コマンドは直列に実行し、成功確認できたタスクだけ `tasks.md` を完了更新する。
- TAKT の `final-ci` ムーブメントではないため、`./scripts/ci-check.sh ai all` は実行しない。

## 影響範囲

- `showcases/std/stream/*`
- `showcases/std/src/support/materializer.rs`
- `showcases/std/tests/stream_showcase_surface.rs`
- `modules/stream-core/tests/public_api_guard.rs`
- `openspec/changes/stream-island-actors/tasks.md`
