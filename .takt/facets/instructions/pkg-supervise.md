パッケージ構造リファクタリングの **最終検証** を行う。`pass_previous_response: false` のため、Report Directory 内のレポートから情報を取得する。

## ピース全体の確認

1. 次のレポートを読み、整合性を確認する:
   - `structure-analysis.md`
   - `structure-design.md`
   - `plan.md`
   - `coder-scope.md` / `coder-decisions.md`
   - `ai-review.md`（あれば）
   - `architect-review.md` / `qa-review.md`
2. **分析 → 設計 → 計画 → 実装** の流れで、目的が達成されているか確認する
3. タスク指示書の要件を **要件ごと** にコードパスで照合する（計画の要約だけを鵜呑みにしない）

## fraktor-rs 固有の検証

- `implement` / `fix` のレポートに、`./scripts/ci-check.sh` 経由の **成功ログ** が記録されていること（タスクで指定されたスコープに合致すること）
- `architect-review.md` / `qa-review.md` が **approved** であること
- **Phase をまたいだ変更がない**こと（本ピースは 1 Phase 単位）
- 削除・移動したモジュールパスが、grep で参照残りしていないこと（可能な範囲で）

supervisor は CI を自身で実行しない。レポート上の証跡で確認する。

## 根本的な設計変更が必要な場合

- 構成案自体の誤り、依存が解決不能、要件と設計が矛盾する場合は **REJECT** とし、次ムーブメントは **design** へ戻すルールに従う（ルール条件: 「根本的な設計変更が必要」）

## Validation 出力契約

`refactoring-supervise` の **Validation 出力契約** に従う（要件充足表・検証サマリー・未完了項目）。

## Summary 出力契約

APPROVE 時は `summary` 契約に従い、確認コマンドとしてタスクで使った `ci-check` を明記する。

## 判定（ルール評価用）

- すべて問題なし → 「すべて問題なし」
- テスト失敗・ビルドエラー・要件未達（修正で解決可能）→ 「テスト失敗、ビルドエラー、要件未達（修正で解決可能）」
- 設計からやり直し → 「根本的な設計変更が必要」
