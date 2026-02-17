# 変更スコープ宣言

## タスク
distinct / distinctBy オペレーター（BTreeSetベースの重複排除フィルタ）を実装

## 変更予定
| 種別 | ファイル |
|------|---------|
| 変更 | `modules/streams/src/core/stage/flow.rs` |
| 変更 | `modules/streams/src/core/stage/stage_kind.rs` |
| 変更 | `modules/streams/src/core/stage/flow/tests.rs` |

## 推定規模
Medium

## 影響範囲
- Flow オペレーター（distinct, distinct_by メソッド追加）
- StageKind（FlowDistinct, FlowDistinctBy 列挙子追加）
- FlowLogic 実装（DistinctLogic, DistinctByLogic 構造体追加）
- 単体テスト（重複排除の振る舞いを検証）
