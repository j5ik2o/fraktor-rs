- すべて日本語でやりとりすること。ソースコード以外の生成されるファイルも日本語で記述すること
- 設計における価値観は "Less is more" と "YAGNI"（要件達成に必要最低限の設計を行い、不要なものを含めない）
- 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- **タスクの完了条件**: テストはすべてパスすること。行うべきテストをコメントアウトしたり無視したりしないこと
- 実装の全タスクを完了した段階で `./scripts/ci-check.sh all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）。実装タスク以外（ドキュメント編集など）は`./scripts/ci-check.sh all`を実行する必要ない
- CHANGELOG.mdはgithub actionが自動的に作るのでAIエージェントは編集してはならない
- lintエラーを安易にallowなどで回避しないこと。allowを付ける場合は人間から許可を得ること

# 基本原則

- シンプルさの優先: すべての変更を可能な限りシンプルに保ち、コードへの影響範囲を最小限に抑える。
- 妥協の排除: 根本原因を特定すること。一時しのぎの修正は行わず、シニア開発者としての基準を維持する。
- 影響の最小化: 必要な箇所のみを変更し、新たなバグの混入を徹底的に防ぐ。

## 設計・命名・構造ルール（.claude/rules/rust/）

詳細は `.claude/rules/rust/` に集約されている。変更する場合は人間から許可を取ること：

| ファイル | 内容 |
|----------|------|
| `immutability-policy.md` | 内部可変性禁止、&mut self 原則、AShared パターン |
| `cqs-principle.md` | CQS 原則、違反判定フロー |
| `type-organization.md` | 1file1type + 例外基準、公開範囲の判断フロー |
| `naming-conventions.md` | 曖昧サフィックス禁止、Shared/Handle 命名、ドキュメント言語 |
| `reference-implementation.md` | protoactor-go/pekko 参照手順、Go/Scala → Rust 変換 |

## Dylint lint（8つ、機械的強制）

mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix

## AI-DLC and Spec-Driven Development
@.agent/CC-SDD.md を読むこと
