## Why

Apache Pekko の参照実装では `pekko-stream`（単数形）を使用している。fraktor-rs の `modules/streams` / `fraktor-streams-rs` は複数形であり、参照実装の命名規約と不一致。

`naming-conventions.md` に「Apache Pekko で確立されたドメイン用語は、プロジェクト内の命名規約より優先する」と定められている。リリース前フェーズのため破壊的変更は歓迎。

## What Changes

- **BREAKING** `modules/streams/` ディレクトリを `modules/stream/` にリネーム
- **BREAKING** クレート名 `fraktor-streams-rs` を `fraktor-stream-rs` にリネーム
- **BREAKING** Rust 識別子 `fraktor_streams_rs` を `fraktor_stream_rs` に変更
- Cargo workspace 定義、CI スクリプト、テスト、examples、doc comments、README のすべての参照を更新
- ルールファイル（`.agents/rules/`、`AGENTS.md`）の記述を更新
- steering / docs / memory 等の設定ドキュメントの記述を一貫性のため更新

## Capabilities

### Modified Capabilities

- `streams-module-naming`: モジュール名・クレート名・ディレクトリ名を Pekko の `stream`（単数形）に統一する

## Impact

- 影響コード: `modules/streams/` 配下の全ファイル（ディレクトリ移動）、`Cargo.toml`（root + module）、`scripts/ci-check.sh`、`scripts/run-pekko-gap-analysis.sh`、テスト・examples 18ファイル、README 2ファイル、ルール・docs 10+ファイル
- 影響 API: `fraktor_streams_rs::*` → `fraktor_stream_rs::*`（クレート名変更）
- 他クレートからの依存: なし（workspace 定義のみ）
- 変更の性質: 機械的な一括置換。ロジック変更なし

## Non-goals

- streams モジュール内のコード・設計の変更（純粋な rename のみ）
- 公開 API の変更（クレート名以外）
