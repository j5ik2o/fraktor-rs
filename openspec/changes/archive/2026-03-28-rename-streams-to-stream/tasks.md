## 1. ディレクトリ・クレートのリネーム

- [x] 1.1 `modules/streams/` を `modules/stream/` にリネーム（`git mv`）
- [x] 1.2 `modules/stream/Cargo.toml` のクレート名を `fraktor-stream-rs` に変更
- [x] 1.3 ルート `Cargo.toml` の workspace members と dependency を更新

## 2. スクリプトの更新

- [x] 2.1 `scripts/ci-check.sh` の `fraktor-streams-rs` → `fraktor-stream-rs` を一括置換
- [x] 2.2 `scripts/run-pekko-gap-analysis.sh` の `"streams"` → `"stream"` を変更

## 3. Rust ソースコードの更新

- [x] 3.1 テスト・examples の `use fraktor_streams_rs::` → `use fraktor_stream_rs::` を一括置換
- [x] 3.2 doc comments 内の `fraktor_streams_rs` → `fraktor_stream_rs` を一括置換

## 4. ドキュメント・ルールの更新

- [x] 4.1 `README.md` / `README.ja.md` の `fraktor-streams-rs` → `fraktor-stream-rs` を一括置換
- [x] 4.2 `.agents/rules/rust/module-structure.md` と `AGENTS.md` の `modules/streams/` → `modules/stream/` を更新
- [x] 4.3 `.kiro/steering/`、`docs/`、`.takt/` の参照を一貫性のため更新

## 5. 検証

- [x] 5.1 `cargo check --workspace` パス
- [x] 5.2 dylint パス、`fraktor-stream-rs` 827テスト全パス
