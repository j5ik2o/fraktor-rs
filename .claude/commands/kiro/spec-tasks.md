---
name: Kiro: Spec Tasks
description: 実装タスクリストを作成する
category: Kiro
tags: [kiro, spec, tasks]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- 要件と設計に基づき、実装可能で検証可能なタスクに分割する。

## 手順
1. `.kiro/specs/<feature>/requirements.md` と `design.md` を確認する。
2. `.kiro/specs/<feature>/tasks.md` を作成または更新する。
3. 既存のタスク形式（チェックボックス、番号、要件参照）に合わせる。
4. 各タスクに `_Requirements: ..._` を付け、要件番号と紐付ける。
5. `spec.json` の `updated_at` と `phase`、`approvals.tasks.generated` を更新する。
