# ギャップ分析: grainref-sender-reply

## 1. 現状調査
- GrainRef 実装: `modules/cluster/src/core/grain_ref.rs`
  - `request` は temp reply actor を生成し、`AnyMessageGeneric::with_sender` で sender を設定して送信
  - reply sender は `GrainReplySender` で future を完了するのみ（sender への転送は未実装）
  - retry/timeout 失敗は `AskError` を `Err` として future に流す仕組みがある
- Cluster API 実装: `modules/cluster/src/core/cluster_api.rs`
  - `request`/`request_future` が `AskResult`（Result）を返す形に更新済み
- メッセージ送信/送信者
  - `AnyMessageGeneric` は `sender` を保持できる
  - `message_invoker` が `sender` を context に反映する（Untyped で sender が参照可能）
- temp actor 登録
  - `SystemState::register_temp_actor` が `/temp/<name>` へ登録し、リモート解決可能な path を持つ
- 例: `modules/cluster/examples/cluster_extension_tokio/main.rs`, `cluster_extension_no_std/main.rs`
  - 現在は `request` / `request_future` を利用（sender 指定 API は未登場）

## 2. 要件から見た必要事項
- sender 指定 API の追加（`tell_with_sender` / `request_with_sender`）
- sender 指定時も Result（成功/失敗）で future を完了させること
- sender へ返信を配送する経路の追加（temp reply actor をプロキシ化 or 新規 sender 追加）
- 既存の retry/timeout/イベント通知の意味を維持
- 例・テストを sender 指定 API と Result 前提で更新

## 3. 要件-資産マップ（ギャップ付き）
| 要件 | 既存資産 | ギャップ/制約 |
| --- | --- | --- |
| 1.1 sender 指定送信 | AnyMessageGeneric::with_sender | GrainRef に API が未実装（Missing） |
| 1.2 Result の Ask 応答 | AskResponse / AskResult | sender 指定 API が未実装（Missing） |
| 1.3 既存解決/送信手順 | GrainRef::resolve_with_retry | 新 API での再利用が未実装（Missing） |
| 1.4 オプション意味維持 | GrainCallOptions / retry | sender 指定 API に統合が未実装（Missing） |
| 1.5 Result 成功の一致 | AskResult / AskError | sender 転送の実装が未実装（Missing） |
| 1.6 Result 失敗の一致 | AskResult / AskError | sender 転送失敗時の規約が未確定（Unknown） |
| 2.1 sender 返信配送 | message_invoker / temp actor | temp reply actor が転送しない（Missing） |
| 2.2 未指定挙動維持 | request/request_future | 現行維持可能（No Gap） |
| 2.3 返信先取り違え防止 | per-call future | sender 指定/未指定混在のテスト不足（Missing） |
| 3.x 失敗時通知 | GrainEvent / EventStream | sender 指定 API への反映が未実装（Missing） |
| 4.x 利用例 | cluster_extension_* | sender 指定 API の例が未整備（Missing） |

## 4. 実装アプローチ案
### Option A: GrainRef 既存拡張（推奨）
- `GrainRefGeneric` に `tell_with_sender` / `request_with_sender` を追加
- temp reply actor を拡張し、返信を sender へ転送しつつ Result を完了
- trade-off:
  - ✅ retry/timeout/イベント通知を既存実装のまま流用
  - ❌ `GrainReplySender` の責務が増える

### Option B: プロキシ専用コンポーネント追加
- sender 転送専用の新規 sender を追加し、future 完了と転送を分離
- GrainRef は新 sender を使って reply と future を管理
- trade-off:
  - ✅ 責務分離が明確でテストしやすい
  - ❌ 新規コンポーネント増加、追加の配線が必要

### Option C: ハイブリッド
- `tell_with_sender` は単純追加
- `request_with_sender` は専用プロキシ導入
- trade-off:
  - ✅ 影響を局所化できる
  - ❌ 実装パスが二重化して一貫性が下がる

## 5. 工数・リスク
- Effort: M（3–7日）
  - API 追加、sender 転送、例/テスト更新が必要
- Risk: Medium
  - sender 指定/未指定混在時の取り違え防止と、転送失敗時の Result/イベント規約が要注意

## 6. デザインフェーズへの提言
- sender 転送失敗を `AskError::SendFailed` に寄せるか、別エラーを用意するかを明記
- temp reply actor をプロキシ化する場合の責務（転送・future 完了・エラー整形）を明文化
- Research Needed:
  - sender 転送失敗時の通知（Result と EventStream の両方にどう反映するか）
