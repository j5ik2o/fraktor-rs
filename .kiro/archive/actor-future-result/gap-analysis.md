# ギャップ分析: actor-future-result

## 1. 現状調査
- Future 実装
  - `modules/actor/src/core/futures/actor_future.rs` / `actor_future_shared.rs` / `actor_future_listener.rs`
  - `ActorFuture<T>` は単一値を保持し、成功/失敗の区別は型として存在しない
  - `ActorFutureSharedGeneric` は `Future` ではなく、`ActorFutureListener` で `Future` 化
- ask 応答
  - `modules/actor/src/core/messaging/ask_response.rs` は `AnyMessageGeneric` をそのまま future に保持
  - `actor_ref::ask` と typed ask (`typed/actor/actor_ref.rs`) も `AnyMessageGeneric` を返す
- 失敗表現
  - `modules/cluster/src/core/grain_ref.rs` / `cluster_api.rs` は `ClusterRequestError` を `AnyMessageGeneric` として future に流す
  - 失敗は EventStream へ通知されるが、future は型として成功/失敗を区別できない
- 影響範囲が広い利用箇所
  - `modules/actor/src/core/system/*` の ask future 管理 (`ask_futures`)
  - `modules/cluster/src/core/*` の request/request_future
  - typed ask (`typed_ask_response`, `typed_ask_future`)
  - examples / tests が多数

## 2. 要件から見た必要事項
- ask の結果を `Result` として返し、成功/失敗を型で区別
- 失敗理由（タイムアウト/配送不能等）を明確に表現
- EventStream の失敗通知は維持
- 既存 API の変更影響を設計で明示

## 3. 要件-資産マップ（ギャップ付き）
| 要件 | 既存資産 | ギャップ/制約 |
| --- | --- | --- |
| 1.1 成功を Result で返す | AskResponse / ActorFuture | ask の出力が `AnyMessage` 固定（Missing） |
| 1.2 失敗を Result で返す | ClusterRequestError 送出 | エラーはメッセージとして混在（Missing） |
| 1.3 失敗の EventStream 通知 | GrainEvent / EventStream | 現行維持可能（No Gap） |
| 2.1 ask 応答ハンドルの Result 化 | AskResponseGeneric | 型変更が必要（Missing） |
| 2.2 API の曖昧さ排除 | typed/untyped ask | 既存 API 全体に波及（Constraint） |
| 2.3 影響範囲明確化 | 設計文書 | 未記載（Missing） |
| 3.1 タイムアウトの失敗表現 | ClusterRequestError::Timeout | メッセージ混在（Missing） |
| 3.2 配送不能の失敗表現 | SendError / DeadLetter | メッセージ混在（Missing） |
| 3.3 成功と混同されない表現 | AnyMessage 返却 | 型で区別できない（Missing） |
| 4.x 利用例 | examples | Result 取り扱いの例が未整備（Missing） |

## 4. 実装アプローチ案
### Option A: AskResponse の結果型を Result 化
- `AskResponseGeneric` を `ActorFutureSharedGeneric<Result<AnyMessageGeneric<TB>, AskError>, TB>` に変更
- typed ask / cluster / system も同じ結果型に統一
- trade-off:
  - ✅ Rust らしい型安全な失敗表現
  - ❌ 影響範囲が広く、破壊的変更が大きい

### Option B: AskOutcome enum を導入
- `AskOutcome`（`Ok(AnyMessage)` / `Err(AskError)`）を future の値として採用
- `Result` と同等の表現力を持ち、独自型で明確化
- trade-off:
  - ✅ 型で成功/失敗を明示
  - ❌ 独自型が増え、API 読み取りコストが上がる

### Option C: 新 API を追加して段階移行
- `ask_result` / `request_result` のような別 API を追加し、Result 化は新 API に限定
- 既存 API を残しつつ段階的に移行
- trade-off:
  - ✅ 互換性を維持しながら移行可能
  - ❌ 二重 API で混乱が増える（YAGNI に反する懸念）

## 5. 工数・リスク
- Effort: L（1–2 週間）
  - core/typed/cluster/system/tests/examples に広範囲の変更が必要
- Risk: Medium
  - 型変更による影響範囲が広く、テスト修正が多い

## 6. デザインフェーズへの提言
- 優先判断: Result 化の方式（Option A/B/C）を確定し、API 影響範囲を設計に明示
- 失敗理由の型（AskError 等）を core に置くか cluster に寄せるかを決める
- Research Needed:
  - protoactor-go / pekko の Future 失敗表現との整合方針
  - typed ask API の Result 表現（typed の reply_to と整合する表現）
