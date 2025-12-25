# リサーチログ

## サマリ
- ask の結果は `AnyMessage` 固定であり、成功/失敗が型で分離されていない
- `AskResponseGeneric` と `ActorFuture` は多数の呼び出し元に波及するため、Result 化は破壊的変更になる
- EventStream 通知は既存経路を維持できる

## リサーチログ
### 1. 既存構造と影響範囲
- `modules/actor/src/core/futures/*` が ask の内部 Future を担う
- `modules/actor/src/core/messaging/ask_response.rs` が ask 応答ハンドルを提供
- typed ask (`typed_ask_response`, `typed_ask_future`) は untyped ask に依存
- cluster (`cluster_api`, `grain_ref`) は `AnyMessage` へエラーを流す設計を前提

### 2. 参照実装の示唆
- protoactor-go の Future は結果とエラーを分離して保持する
- Pekko の Future も成功/失敗を型で区別する設計

## アーキテクチャ検討
- ask の future 値を `Result<AnyMessage, AskError>` に統一することで成功/失敗を明確化できる
- EventStream は観測経路として残し、結果の主経路は Result に統一する

## リスクと対策
- 破壊的変更による広範囲の影響 → 設計で影響範囲と移行方針を明記
- typed ask への波及 → 型変換の責務を明確化して混乱を抑える
