# 実装計画

## 対象
- `AI-REV-003`
- `modules/actor-core/src/core/typed/dsl/routing/pool_router.rs`

## 方針
- `select_consistent_hash_index` の `unwrap_or(0)` を除去する
- 呼び出し前提で保証されている不変条件は既定値へ落とさず、違反時に即座に顕在化させる
- 修正は `PoolRouter` の consistent hash helper に限定し、routing 戦略全体には波及させない

## 実装手順
1. `select_consistent_hash_index` の index 解決を `expect(...)` ベースへ置き換える
2. 既存テストで該当 helper の経路を確認する
3. 変更範囲に限定した Rust テストを実行する
