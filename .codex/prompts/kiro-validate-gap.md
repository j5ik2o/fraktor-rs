---
description: 既存コードと要件のギャップ分析を作成する
argument-hint: feature-name
---

$ARGUMENTS

## 目的
- 既存資産と要件の差分を整理し、設計フェーズの判断材料を作る。

## 手順
1. `.kiro/specs/<feature>/requirements.md` と既存コードを確認する。
2. `.kiro/specs/<feature>/gap-analysis.md` を作成または更新する。
3. 現状調査、要件から見た必要事項、要件-資産マップ、実装アプローチ案を記述する。
4. 言語は `spec.json.language` に合わせる。

## 成果物
- `.kiro/specs/<feature>/gap-analysis.md`
