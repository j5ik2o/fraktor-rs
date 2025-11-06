# 提案: Create ACK ハンドシェイクの撤廃

## Why
- `SystemMessage::Create` の完了を dispatcher 側の ACK future で待つ現在の実装は、`ActorSystem::spawn_with_parent` が親スレッドを同期的にブロックし、fire-and-forget を前提とするアクターモデルの原則に反している。
- enqueue 成功か失敗かのみを保証すべき `tell` 相当 API が `pre_start` 実行結果まで同期的に伝搬すると、Supervisor や親アクターのスループットが一気に低下し、protoactor-go / Pekko に存在しない待機コストが発生する。
- ACK future を提供するために `ActorCell` と `ActorSystem` 間でロック共有や busy-spin を行っており、no_std / bare-metal 環境での電力・CPU コストも増大している。

## What Changes
- `ActorSystem::spawn_with_parent` から Create ACK future の生成・待機を取り除き、`SystemMessage::Create` が enqueue に成功した時点で `ChildRef` を返す。
- `ActorCell` 内の `pending_create_ack` フィールドと `prepare_create_ack` / `notify_create_result` を削除し、`SystemMessage::Create` の完了通知は EventStream / LifecycleEvent に一本化する。
- `spawn` フローで `SystemMessage::Create` の enqueue に失敗した場合のみ即時 `SpawnError` を返し、`pre_start` 失敗は Supervisor / EventStream など二次経路で観測させるよう仕様を更新する。
- README および関連ドキュメントから「dispatcher ACK を待機する」といった説明を削除し、起動失敗時の検知方法（LifecycleEvent 監視など）を追記する。

## Impact
- 影響スペック: `001-add-actor-runtime`（`ActorSystem`/ライフサイクル管理要件の更新）
- 影響コード: `modules/actor-core/src/system/base.rs`, `modules/actor-core/src/actor_prim/actor_cell.rs`, 付随テスト、README 等
- 依存中の変更: なし（`add-system-message-failure` とは競合しないが、同じ Lifecycle 経路を触るためレビュー時に差分確認が必要）
