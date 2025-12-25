# ギャップ分析: grainref-sender-reply

## 1. 現状調査
- GrainRef 実装: `modules/cluster/src/core/grain_ref.rs`
  - `request` は temp reply actor を生成し、`AnyMessageGeneric::with_sender` で sender を設定して送信
  - タイムアウト/リトライは temp reply actor を前提に再送・クリーンアップ
- メッセージと sender 伝搬: `modules/actor/src/core/messaging/any_message.rs`, `modules/actor/src/core/messaging/message_invoker/pipeline.rs`
  - sender をメッセージに保持し、処理中の `ActorContext` に反映
- ask 応答ハンドル: `modules/actor/src/core/messaging/ask_response.rs`
  - sender（返信先）と future をペアで保持
- 例: `modules/cluster/examples/cluster_extension_tokio/main.rs`, `modules/cluster/examples/cluster_extension_no_std/main.rs`
  - `request` / `request_future` を使用

## 2. 要件から見た必要事項
- GrainRef に sender を明示できる API（`tell_with_sender`, `request_with_sender`）が必要
- sender はユーザが用意したアクター参照であることを保証
- sender 指定時も、解決・送信・タイムアウト・リトライ・失敗通知が既存と同等であること
- sender 指定時に Ask 応答ハンドルを返す要件がある

## 3. 要件-資産マップ（ギャップ付き）
| 要件 | 既存資産 | ギャップ/制約 |
| --- | --- | --- |
| 1.1 送信者指定送信 | `AnyMessageGeneric::with_sender`, `ActorRefGeneric::tell` | GrainRef に sender 指定 API が未実装（Missing） |
| 1.2 sender 指定 ask 応答 | `AskResponseGeneric`, `ActorFutureSharedGeneric` | sender をユーザ参照に固定したまま future を返す経路が未定義（Unknown） |
| 1.3 既存解決・送信手順の踏襲 | `GrainRefGeneric::resolve_with_retry`, `request` | 新APIで既存手順を再利用する経路は設計が必要（Missing） |
| 1.4 既存オプションの意味維持 | `GrainCallOptions`, `GrainRetryRunnable` | リトライ/タイムアウトが temp reply actor 前提のため sender 指定時の意味付けが未定義（Constraint） |
| 2.1 sender 返信の配送 | `message_invoker` による sender 伝搬 | sender がユーザ参照の場合の配送経路は既存 ActorRef に依存（Constraint） |
| 2.2 sender 未指定の既存挙動 | `GrainRefGeneric::request` | 既存実装が維持できる（No Gap） |
| 2.3 返信先の取り違え防止 | `AnyMessageGeneric` が sender を個別保持 | sender 指定と未指定の混在に対する明示テストが不足（Missing） |
| 3.x 失敗時の通知 | `GrainEvent::CallFailed`, `record_call_failed` | sender 指定時も同経路を使えるが API 経由での確認が必要（Missing） |
| 4.x 利用例 | 既存 examples | sender 指定 API を示す例が未整備（Missing） |

## 4. 実装アプローチ案
### Option A: GrainRef 既存拡張
- 既存 `GrainRefGeneric` に `tell_with_sender` / `request_with_sender` を追加
- sender 指定時の retry/timeout は既存ロジックを流用
- trade-off:
  - ✅ 変更範囲が限定的
  - ❌ sender 指定時の Ask 応答ハンドルの扱いが複雑化しやすい

### Option B: 返信ブリッジ専用コンポーネント追加
- sender 指定 ask に対応する「返信ブリッジ（reply forwarder）」を新規追加
- GrainRef は新コンポーネント経由で sender と future の整合を取る
- trade-off:
  - ✅ 返信経路の責務分離が明確
  - ❌ 新規ファイルと API を増やす必要

### Option C: ハイブリッド
- `tell_with_sender` は単純追加
- `request_with_sender` は返信ブリッジ（最小限）を追加
- trade-off:
  - ✅ 要件を満たしつつ影響を局所化
  - ❌ 実装パスが 2 系統になり理解コストが増える

## 5. 工数・リスク
- Effort: M（3–7日）
  - GrainRef/例/テストの更新と sender 指定 ask の設計検討が必要
- Risk: Medium
  - sender 指定時の Ask 応答ハンドルの意味付けが曖昧で、誤実装の可能性がある

## 6. デザインフェーズへの提言
- 優先判断: `request_with_sender` が返す Ask 応答ハンドルの意味（誰が受け取るか）を明確化する
- 既存の temp reply actor を sender にしない前提で、返信の「待機」と「転送」の責務分担を決める
- Research Needed:
  - sender 指定 ask の合意仕様（future 完了条件、転送の責務）
  - sender がリモートアクターの場合の配送保証と失敗時挙動
