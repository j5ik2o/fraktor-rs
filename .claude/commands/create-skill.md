---
name: Skills: Create
description: スキル定義ファイルを作成する
category: Skills
tags: [skills]
---

## ユーザー入力

```
$ARGUMENTS
```

## 目的
- 追加するスキルの目的・入力・出力・注意点を明確にし、複数の実行環境で共通利用できる形にする。

## 手順
1. 入力からスキル名（kebab-case 推奨）と対象範囲を確定する。
2. `.claude/skills/<skill>.md` を作成または更新する。
3. `.codex/skills/<skill>.md` と `.agent/skills/<skill>.md` に同一内容を同期させる。
4. 先頭の YAML に `name` と `description` を必ず含める。
5. スキル本文は日本語で簡潔に記述し、運用上の注意点を明記する。
