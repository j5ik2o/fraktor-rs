---
name: Kiro: Spec Design
description: 設計ドキュメントを作成する
category: Kiro
tags: [kiro, spec, design]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- 要件に対する解決方針と実装方針を明確にし、影響範囲とトレーサビリティを可視化する。

## 手順
1. `.kiro/specs/<feature>/requirements.md` と `spec.json` を確認する。
2. `.kiro/steering/` の制約に従い、最小限の設計とする。
3. `.kiro/specs/<feature>/design.md` を作成または更新する。
4. 既存の設計ドキュメント構成（概要、目標/非目標、アーキテクチャ、フロー、要件トレーサビリティ）に合わせる。
5. `spec.json` の `updated_at` と `phase`、`approvals.design.generated` を更新する。
