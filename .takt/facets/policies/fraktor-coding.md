# fraktor-rs コーディングポリシー

## 言語

- レポート・分析・コメントはすべて日本語で記述すること
- rustdoc（`///`, `//!`）のみ英語

## Dylint リント（8つ、機械的強制）

mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix

編集前に `./scripts/ci-check.sh dylint -m <module>` を実行すること。

## CQS 原則

- Query: `&self` + 戻り値
- Command: `&mut self` + `()` or `Result<(), E>`
- 違反する場合は人間の許可を得ること

## 内部可変性ポリシー

- デフォルト禁止。`&mut self` で設計
- 共有が必要な場合のみ AShared パターン（`ArcShared<ToolboxMutex<T>>`）

## 命名規約

- 禁止サフィックス: Manager, Util, Facade, Service, Runtime, Engine
- Shared/Handle 命名ルールに従う
- rustdoc は英語、それ以外は日本語

## Pekko → Rust 変換ルール

- Scala trait 階層 → Rust trait + 合成
- Scala implicit → Rust ジェネリクス + RuntimeToolbox
- sealed trait + case classes → enum

## テストポリシー

- テストは `{type}/tests.rs` に配置
- テストをコメントアウトや無視しない
- 全タスク完了時は `./scripts/ci-check.sh all` を通す

## REJECT 基準

- `#[allow]` による lint 回避（人間許可なし）
- 内部可変性の無断使用
- 1ファイル複数公開型（lint 違反）
- テストなしの実装
- 後方互換のための不要なコード
