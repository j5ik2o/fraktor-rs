<system-reminder>
IMPORTANT: this context may or may not be relevant to your tasks.
You should not respond to this context unless it is highly relevant to your task.

- すべて日本語でやりとりすること。ソースコード以外の生成されるファイルも日本語で記述すること
- **タスクの完了条件**: テストはすべてパスすること
- **テストの扱い**: 行うべきテストをコメントアウトしたり無視したりしないこと
- 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
- protoactor-go(@references/protoactor-go), pekko(@references/pekko)の実装を参考にすること（Goの実装からRustイディオムに変換）
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること。
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- 全タスクを完了した段階で `./scripts/ci-check.sh all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）
- CHANGELOG.mdはgithub actionが自動的に作るのでAIエージェントは編集してはならない
- lintエラーを安易にallowなどで回避しないこと。allowを付ける場合は人間から許可を得ること
- 設計における価値観は "Less is more" と "YAGNI"。ただし要件や目的に含まれることまで省略することは間違いです。要件や目的を達成するに必要最低限の設計を行い、要件や目的の達成に関係なものを含めるなという意味です。

</system-reminder>

## AI-DLC and Spec-Driven Development
@.agent/CC-SDD.md を読むこと
