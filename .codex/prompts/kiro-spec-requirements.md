---
description: 要件ドキュメントを作成する
argument-hint: feature-name
---

$ARGUMENTS

## 目的
- 仕様対象の目的と受け入れ条件を明確化し、以降の設計・実装の基盤にする。

## 手順
1. `.kiro/specs/<feature>/spec.json` を読み、言語とフェーズを確認する。
2. `.kiro/steering/` の方針を反映する。
3. `.kiro/specs/<feature>/requirements.md` を作成または更新する。
4. 既存の仕様ドキュメント構成（導入、要件、目的、受け入れ条件）に合わせて記述する。
5. `spec.json` の `updated_at` と `phase`、`approvals.requirements.generated` を更新する。

## 成果物
- `.kiro/specs/<feature>/requirements.md`
