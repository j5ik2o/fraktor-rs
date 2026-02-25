複数の GitHub Issue を実装可能な作業計画に分解してください。

## 手順

1. User Request と `order.md` から issue 番号一覧（`#123` 形式）を抽出する
2. issue ごとに目的・制約・受け入れ条件を定義する
3. issue ごとに実装対象ファイルと非対象（スコープ外）を明確化する
4. issue ごとの完了順序を決める
5. issue ごとに仮のコミットメッセージ案を作る
6. issue ごとに `./scripts/ci-check.sh all` 実行タイミングを計画に含める
7. 事前にコミット方針を明記する（Issue単位で1コミット以上）

## 重要

- 仕様にない拡張は提案しない（YAGNI）
- 不明点は「前提不足」として明記する
- 受け入れ条件が定義できない issue が1件でもあれば ABORT 判定できる情報を残す
- コミットメッセージは Conventional Commits かつ英語であることを計画段階で明記する
- コミットメッセージ案は `(#<issue-number>)` を末尾につける
- 各 issue 完了時点で `./scripts/ci-check.sh all` を実行する前提を明記する
