<system-reminder>
IMPORTANT: this context may or may not be relevant to your tasks.
You should not respond to this context unless it is highly relevant to your task.

- すべて日本語でやりとりすること。ソースコード以外の生成されるファイルも日本語で記述すること
- **コメント言語**: rustdoc(`///`や`//!`)は英語、それ以外のコメントやドキュメントは日本語で記述すること
- **タスクの完了条件**: テストはすべてパスすること
- **テストの扱い**: 行うべきテストをコメントアウトしたり無視したりしないこと
- 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
- protoactor-go(@references/protoactor-go), pekko(@references/pekko)の実装を参考にすること（Goの実装からRustイディオムに変換）
- ランタイム本体で `#[cfg(feature = "std")]` による機能分岐を入れないこと（テストコード内での使用は許容）
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること。
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- mod.rs禁止。2018モジュールを使え
- 単体テストは hoge.rs に対して hoge/tests.rs に記述すること
- 1ファイルに複数構造体、複数trait、複数enumを記述しないこと(ただしプライベートな構造体・trait・enumは対象外)
- 全タスクを完了した段階で `./scripts/ci-check.sh all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）
- CHANGELOG.mdはgithub actionが自動的に作るのでAIエージェントは編集してはならない
- lintエラーを安易にallowなどで回避しないこと。allowを付ける場合は人間から許可を得ること
- 設計における価値観は "Less is more" と "YAGNI"。ただし要件や目的に含まれることまで省略することは間違いです。要件や目的を達成するに必要最低限の設計を行い、要件や目的の達成に関係なものを含めるなという意味です。
- 内部可変性をデフォルトでは禁止する。可変操作はまず&mut selfで設計すること。なんでもかんでも&selfメソッド+内部可変性とするとRustらしさが失われます。
- traitにある&mut selfメソッドはセマンティクスを重視した設計(戻り値を返さないで状態を変えるメソッドは&selfではなく&mut selfが原則です)になっているので、安易に&selfメソッド+内部可変性にリファクタリングしないこと。変更する場合は人間から許可を取ること
- &mut selfなメソッドを持つ型Aが共有される場合は、innerにArc<ToolboxMutex<A>>を保持するASharedを新設すること。つまり内部可変性はこのときだけ許容されます。具体的にはこのガイドに従うこと docs/guides/shared_vs_handle.md
- CQSをできるだけ守ること、消極的に違反せざるを得ないときは、人間の許可を取ること（Vec::popは読み取りだけど更新が必要なので、CQS違反しないとロジックが書けないところがあるが極力CQSを守るようにすること）

</system-reminder>

## AI-DLC and Spec-Driven Development

@.agent/CC_SDD-CLAUDE.md を読むこと
