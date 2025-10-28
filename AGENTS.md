<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

## 重要な注意事項

- **応対言語**: 必ず日本語で応対すること
- **コメント言語**: rustdoc(`///`や`//!`)は英語、それ以外のコメントやドキュメントは日本語で記述すること
- **タスクの完了条件**: テストはすべてパスすること
- **テストの扱い**: 行うべきテストをコメントアウトしたり無視したりしないこと
- **実装方針**:
    - 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
    - protoactor-go(@references/protoactor-go), pekko(@references/pekko)の実装を参考にすること（Goの実装からRustイディオムに変換）
- ランタイム本体で `#[cfg(feature = "std")]` による機能分岐を入れないこと（テストコード内での使用は許容）
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること。
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- mod.rs禁止。2018モジュールを使え
- 単体テストは hoge.rs に対して hoge/tests.rs に記述すること
- 1ファイルに複数構造体、複数traitを記述しないこと
- 作業後に`./scripts/ci-check.sh all`を実行し、エラーがないことを確認すること
