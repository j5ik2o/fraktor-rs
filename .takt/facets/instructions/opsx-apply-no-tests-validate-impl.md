実装が設計・タスクに合致しているかを検証せよ。

**注意:** OpenSpec の変更ディレクトリ（`openspec/changes/{change}/`）に設計・タスクが揃っていない場合は ABORT する。

**やること:**
1. レポートディレクトリの 00-plan.md から対象変更を特定する
2. `openspec/changes/{change}/` の成果物を読み込む:
   - `proposal.md` - 提案
   - `design.md` - 設計
   - `tasks.md` - タスクリスト
3. タスク完了状況を確認する:
   - `tasks.md` のチェックボックス `[x]` を数える
   - 未完了タスクを特定する
4. 設計整合性を検証する:
   - 設計のディレクトリ構造・module path・公開 import path と実装が一致するか確認する
   - `core/kernel` と `core/typed` の境界、および `typed` 内の package 分割が設計どおりか確認する
5. Dylint確認:
   - `./scripts/ci-check.sh ai dylint` を実行し、構造変更による module wiring / dylint 破綻がないことを確認する
6. 実装運用の確認:
   - レポートから、実装 movement が `./scripts/ci-check.sh ai dylint` を編集単位で実施しているか確認する

**注意: テストについて**
- このピースはテスト先行が不要なタスク（package/module構造変更、examples作成等）向け
- テスト実行ではなく、構造変更に適した `./scripts/ci-check.sh ai dylint` による確認を行う
- `final-ci` 以外では `./scripts/ci-check.sh ai all` を実行してはならない

**必須出力（見出しを含める）**
## タスク完了状況
- 完了: {N} / 全体: {M}
- 未完了タスク: {一覧}
## 設計整合性
- {設計との一致/不一致の詳細}
## Dylint結果
- {実行コマンドと結果}
## 判定
- {検証合格 / 検証不合格}
