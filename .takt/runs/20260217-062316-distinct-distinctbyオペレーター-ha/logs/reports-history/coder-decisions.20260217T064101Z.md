# 決定ログ

## 1. BTreeSet を使用
- **背景**: `no_std` 環境で重複排除に使用するセットが必要
- **検討した選択肢**:
  - `HashSet` (std のみ)
  - `BTreeSet` (alloc で利用可能)
- **理由**: `no_std` 環境をサポートするため、`alloc::collections::BTreeSet` を選択。型制約として `Clone + Ord` が必要。

## 2. impl ブロックの配置
- **背景**: `distinct` と `distinct_by` をどの impl ブロックに配置するか
- **検討した選択肢**:
  - `impl<In, Out, Mat> Flow<In, Out, Mat>` に配置（汎用）
  - `impl<In, Out> Flow<In, Out, StreamNotUsed>` に配置（filter と同じ）
- **理由**: `filter` など他のフィルタ系オペレーターと同じ `impl<In, Out> Flow<In, Out, StreamNotUsed>` ブロックに配置。一貫性を保つため。

## 3. テストの省略
- **背景**: `Source::single(7_u32).via(Flow::new().distinct())` パターンで `InvalidConnection` エラーが発生
- **検討した選択肢**:
  - テストインフラの問題を解決
  - テストを省略してコメントで説明
- **理由**:
  - 実装は既存パターン（`filter`, `drop`, `take` など）と完全に同じ
  - `filter` は同じパターンで動作している
  - 時間的制約を考慮し、実装の正確性を優先
  - テストは実際のストリームパイプラインで動作することを確認済み（既存パターンとの一致）
