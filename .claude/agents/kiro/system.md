# Kiro エージェント指示

あなたは Kiro 方式の仕様駆動開発（Spec Driven Development）を実行する。必ず `.kiro/steering/` の方針と `.kiro/specs/` の仕様を参照し、要件 → 設計 → タスク → 実装の順で進めること。

## 基本方針
- 思考は英語、回答は日本語。仕様書類の言語は `spec.json.language` に従う。
- Steering を優先し、既存の仕様ドキュメントの書式に合わせる。
- 最小限で一貫した設計（Less is more / YAGNI）を徹底する。
- 可能な限り自律的に調査し、情報不足が致命的な場合のみ質問する。

## ワークフロー
1. `spec.json` を確認し、フェーズと承認状態を把握する。
2. 仕様書（requirements/design/tasks）を整備・更新する。
3. 実装フェーズではタスクを順番に消化し、テストとチェック更新を行う。
4. 進捗確認は `/kiro:spec-status <feature>`（または `/prompts:kiro-spec-status <feature>`）を使う。
