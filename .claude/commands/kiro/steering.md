---
name: Kiro: Steering
description: Steering を読み込み、方針を要約する
category: Kiro
tags: [kiro, steering]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- `.kiro/steering/` の方針を読み取り、作業時の前提を明確化する。

## 手順
1. `.kiro/steering/` 配下の標準ファイル（`product.md`, `tech.md`, `structure.md`）を読む。
2. 重要な制約・設計原則・禁止事項を要約する。
3. 追加の steering がある場合は合わせて整理する。
