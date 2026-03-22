# fraktor-rs コーディングポリシー

## 言語

- レポート・分析・コメントはすべて日本語で記述すること
- rustdoc（`///`, `//!`）のみ英語

## Dylint リント（8つ、機械的強制）

mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix

編集前に対象範囲のlintを実行すること。実行コマンドは各ピースの instruction 指示を優先する。

## CQS 原則

- Query: `&self` + 戻り値
- Command: `&mut self` + `()` or `Result<(), E>`
- 違反する場合は人間の許可を得ること

## 内部可変性ポリシー

- デフォルト禁止。`&mut self` で設計
- 共有が必要な場合のみ AShared パターン（`ArcShared<SpinSyncMutex<T>>`）

## 命名規約

- 禁止サフィックス: Manager, Util, Facade, Service, Runtime, Engine
- Shared/Handle 命名ルールに従う
- rustdoc は英語、それ以外は日本語

## Pekko → Rust 変換ルール

- Scala trait 階層 → Rust trait + 合成
- Scala implicit → Rust ジェネリクスまたは通常の引数
- sealed trait + case classes → enum

## テストポリシー

- テストは `{type}/tests.rs` に配置
- テストをコメントアウトや無視しない
- 全タスク完了時は各ピースで定義された最終CIゲートを通す

## CI 実行制限

- `./scripts/ci-check.sh ai all` は `final-ci` ムーブメント専用。他のムーブメントでは実行禁止
- 変更範囲に限定した単体版（例: `./scripts/ci-check.sh ai dylint -m モジュール名`）は許可

## アーティファクト配置ルール

- takt ピース実行中に生成するレポート・計画・分析・決定ログ等の中間アーティファクトは **`.takt/` 配下にのみ**配置すること
- プロジェクトルート直下やソースツリー内（`reports/`, `docs/plans/` 等）に中間アーティファクトを書き出してはならない
- ソースコードの編集（`modules/`, `showcases-std/` 等）はこの制約の対象外

## REJECT 基準

- `#[allow]` による lint 回避（人間許可なし）
- 内部可変性の無断使用
- 1ファイル複数公開型（lint 違反）
- テストなしの実装
- 後方互換のための不要なコード
- `./scripts/ci-check.sh ai all` の `final-ci` 以外での実行
- `.takt/` 外への中間アーティファクト配置
