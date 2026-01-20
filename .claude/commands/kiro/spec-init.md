---
name: Kiro: Spec Init
description: 仕様フォルダと spec.json を初期化する
category: Kiro
tags: [kiro, spec, init]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- 新規仕様のベースを作成し、以後のフェーズに備える。

## 手順
1. 入力から feature 名を確定し、kebab-case で命名する。
2. `.kiro/steering/` を読み、制約とルールを把握する。
3. `.kiro/specs/<feature>/` を作成する。
4. `spec.json` を作成・更新し、`feature_name`/`created_at`/`updated_at`/`language`/`phase`/`approvals` を設定する。
5. `language` は指定がなければ `ja` とし、ドキュメント言語に反映する。
