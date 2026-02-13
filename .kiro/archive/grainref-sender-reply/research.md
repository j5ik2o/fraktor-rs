# リサーチログ

## サマリ
- 既存の GrainRef は temp reply actor を前提にしており、sender 指定 API は未実装
- 送信者指定の追加は `GrainRefGeneric` の拡張が最短経路
- 新規依存は不要で、既存の sender 伝搬と EventStream を再利用できる

## リサーチログ
### 1. 既存拡張点と統合面
- 対象ファイル: `modules/cluster/src/core/grain_ref.rs`
- 既存の `request` は temp reply actor を生成し、`AnyMessageGeneric::with_sender` で sender を設定
- retry/timeout も temp reply actor を前提に再送/クリーンアップを行う
- `AnyMessageGeneric` は sender を保持し、`message_invoker` で `ActorContext` に反映される

### 2. 依存関係・互換性
- 新規依存ライブラリは不要
- no_std/std 境界は既存の RuntimeToolbox を踏襲

## アーキテクチャ検討
- 既存 `GrainReplySender` に「転送先 sender」を追加することで temp reply actor をプロキシ化できる
- sender 指定の `request_with_sender` は既存の解決・送信・retry/timeout フローを再利用するのが最小変更

## リスクと対策
- sender 指定時に返信が二重配送になる誤実装リスク → 設計で「返信は sender と future に同一内容を送る」と明記
- 混在呼び出し時の取り違え → テストで sender 指定/未指定の同時実行を検証
