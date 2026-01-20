---
name: Kiro: Validate Design
description: 設計の妥当性を確認する
category: Kiro
tags: [kiro, validate, design]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- 設計が要件と Steering に整合しているかを点検する。

## 手順
1. `.kiro/specs/<feature>/requirements.md` と `design.md` を確認する。
2. 主要な整合性・欠落・リスクを洗い出す。
3. 問題がなければ承認待ちのサマリを提示する。
4. 承認が得られた場合のみ `spec.json.approvals.design.approved` を更新する。
