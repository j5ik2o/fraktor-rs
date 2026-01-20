---
name: Kiro SDD
description: Kiro 方式の仕様駆動開発ガイド
---

# Kiro 仕様駆動開発スキル

## 目的
- 仕様（requirements/design/tasks）を先に固め、実装とテストのトレーサビリティを確保する。
- `.kiro/steering/` の方針に従い、最小限で一貫した設計と実装を行う。

## 進め方
1. Steering を読み、全体方針と制約を把握する。
2. `.kiro/specs/<feature>/` に仕様書を作成する。
3. `spec.json.language` に合わせてドキュメント言語を統一する。
4. 要件 → 設計 → タスク → 実装の順で進め、各フェーズで人間レビューを受ける。

## 注意点
- コマンドは環境により `/kiro:` または `/prompts:` を使う。
- タスクは要件番号を紐付け、完了時にチェックを更新する。
