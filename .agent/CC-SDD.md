# AI-DLC と Spec Driven Development

AI-DLC（AI Development Life Cycle）上で Kiro 方式の Spec Driven Development を行うための共通ガイド。

## プロジェクトコンテキスト

### パス
- Steering: `.kiro/steering/`
- Specs: `.kiro/specs/`

### Steering と Specification
- **Steering**: プロジェクト全体の方針・規約・前提を示す
- **Specs**: 機能ごとの仕様策定と実装手順を示す

### アクティブ仕様
- `.kiro/specs/` を確認する
- 進捗確認: `/kiro:spec-status <feature>` または `/prompts:kiro-spec-status <feature>`

## 開発ガイドライン
- 思考は英語、回答は日本語。仕様書類の言語は `spec.json.language` に従う。

## 最小ワークフロー
- Phase 0（任意）: `/kiro:steering` `/kiro:steering-custom`
- Phase 1（Specification）:
  - `/kiro:spec-init "description"`
  - `/kiro:spec-requirements <feature>`
  - `/kiro:validate-gap <feature>`（任意）
  - `/kiro:spec-design <feature> [-y]`
  - `/kiro:validate-design <feature>`（任意）
  - `/kiro:spec-tasks <feature> [-y]`
- Phase 2（Implementation）: `/kiro:spec-impl <feature> [tasks]`
  - `/kiro:validate-impl <feature>`（任意）
- 進捗確認: `/kiro:spec-status <feature>`

※環境によっては `/prompts:` プレフィックスを使用する。

## ルール
- 要件 → 設計 → タスク → 実装の3段階承認フロー
- 各フェーズで人間レビューが必要（`-y` は意図的な高速化のみ）
- Steering を最新化し、`spec-status` で整合性を確認
- ユーザー指示に従い、必要な情報を自律的に収集して完遂する

## Steering の読み込み
- `.kiro/steering/` をプロジェクトメモリとして読み込む
- デフォルト: `product.md` `tech.md` `structure.md`
- カスタムは `/kiro:steering-custom` で管理
