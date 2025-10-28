<!--
Sync Impact Report
Version change: 1.1.0 → 1.1.1
Modified principles:
- なし
Added sections:
- なし
Removed sections:
- なし
Templates requiring updates:
- ✅ .specify/templates/plan-template.md
- ✅ .specify/templates/tasks-template.md
- ⚠ .specify/templates/spec-template.md（変更不要だが次回レビュー時に整合性確認）
- ⚠ .specify/templates/commands/*.md（対象ファイルなしのため確認のみ）
Follow-up TODOs:
- なし
-->

# cellactor-rs Constitution

## Core Principles

### I. no_stdコアの維持
- `modules/*-core` クレートは常に `#![no_std]` でビルド可能でなければならない。  
- `std` への依存は `cfg(test)` またはホスト専用クレートに限定し、ランタイム本体に `#[cfg(feature = "std")]` を導入してはならない。  
- `modules/*-core` では `tokio` や `embassy` を含むランタイムクレートを依存に追加してはならない。`tokio` 依存は `*-std`、`embassy` 依存は `*-embedded` クレートに隔離する。  
- `std` が必要な機能はアダプタ層を別クレートで提供し、境界を明確化すること。  
理由: 組込みターゲットでの動作保証と決定論的挙動を維持するため。

### II. テスト完全性とCI厳守
- 各ユーザーストーリーは失敗するテストから実装を開始し、完了時に `./scripts/ci-check.sh all` をグリーンに保つ。  
- テストのコメントアウト・`#[ignore]` 化・削除は禁止する。  
- テスト結果は計画/タスク文書に記録し、失敗時は原因と対策を明示する。  
理由: 品質を定量的に担保し、CIで早期に退行を検出するため。

### III. リファレンス整合設計
- protoactor-go と Apache Pekko の該当実装を調査し、設計意図を Rust イディオムへ変換して取り込む。  
- 参考実装との差分を文書化し、意図的な乖離には根拠とフォローアップを提示する。  
- 参照結果は spec/plan/tasks に反映し、変更履歴を追跡可能にする。  
理由: 既存の成熟したアクターランタイムの知見を再利用し、設計の一貫性を守るため。

### IV. モジュール構造と型隔離
- 2018 モジュールシステムを採用し、`mod.rs` を使用しない。  
- 1 ファイルには 1 つの構造体または 1 つの trait のみを定義し、単体テストは `target/tests.rs` 形式で分離する。  
- 内部では FQCN による `use` を基本とし、`prelude` は外部公開向けに限定する。  
- `docs/guides/module_wiring.md` の規約とカスタム lint（module-wiring, type-per-file 等）を満たすこと。  
理由: モジュール可読性とビルド規律を保ち、tooling による自動検証を成立させるため。

### V. 攻めの設計進化
- まだプレリリース段階であるため、最適な設計を優先し破壊的変更を恐れない。  
- 破壊的変更は spec/plan/tasks で移行手順と影響範囲を先に定義し、レビュアブルな形で提示する。  
- 変更後は関連ドキュメントとテンプレートの更新を即座に実施する。  
理由: フィードバックループを短縮し、将来の負債を抑制するため。

## 実装規約と構造制約

- `modules/actor-core` と `modules/utils-core` は `alloc` 系ライブラリを活用しつつ `#![no_std]` を徹底し、`panic-halt` などの組込み向け設定を維持する。  
- `tokio` や `embassy` のようなランタイム依存は `modules/*-core` から排除し、対応する `*-std` / `*-embedded` クレートに限定する。  
- テストやベンチマークで `std` を利用する場合でも `cfg(test)` や専用クレート内に封じ込める。  
- 単体テストは対象モジュールと同階層の `tests.rs` にまとめ、共通ヘルパーは別モジュールに分離する。  
- rustdoc（`///`, `//!`）は英語で記述し、それ以外のコメント・ドキュメントは日本語で記述する。  
- FQCN による `use` を基本とし、再エクスポートは末端モジュールの直属親に限定する。  
- `lints/` 配下のカスタム lint は `lints/*/README.md` に明文化されたルールを参照し、作業前後で `makers ci-check -- dylint` を実行して逸脱を検知する。AI や CI が自動修正を試みる前に必ず内容を更新・確認する。

## 開発フローとレビュー手順

- 新規機能や破壊的変更は OpenSpec ワークフローで plan/spec/tasks を整備し、憲章チェックを通過してから実装を開始する。  
- コード調査・編集には Serena MCP ツールを使用し、取得した知見を記録として残す。  
- すべての作業は `./scripts/ci-check.sh all` と `makers ci-check -- dylint` の双方を完了条件とし、結果ログをレビューに添付する。  
- 重大な差分は protoactor-go / Apache Pekko の比較結果とともにレビューに提示する。  
- 完了したユーザーストーリーごとにテスト結果とドキュメント更新を報告し、段階的なデモ/デプロイを可能にする。

## Governance

- この憲章は cellactor-rs の開発規範の唯一の出典であり、他ドキュメントより優先される。  
- 改定は OpenSpec プロポーザルの承認を前提とし、変更理由・影響分析・テンプレート更新計画を含めなければならない。  
- バージョニングは SemVer に従う: 原則の差し替えや削除は MAJOR、原則追加や大幅な運用指針拡張は MINOR、表現調整や軽微な補足は PATCH を増分する。  
- すべての PR は憲章遵守チェックリストをレビューコメントに添付し、CI 結果とテスト稼働ログを含める。  
- 四半期ごとにメンテナが遵守状況を棚卸しし、逸脱があれば是正計画と期日を記録する。

**Version**: 1.1.1 | **Ratified**: 2025-10-28 | **Last Amended**: 2025-10-28
