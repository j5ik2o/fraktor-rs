# デザイン: Create ACK 非同期化

## 背景
- 現状の `ActorSystem::spawn_with_parent` は dispatcher スレッドからの Create 完了 ACK を待機し、親スレッドが busy-spin する。
- アクターモデルでは `tell` の戻り値は enqueue 成否までが責務であり、`pre_start` の結果を同期的に返す設計は fire-and-forget を破壊する。
- no_std/bare-metal では busy-spin が CPU を占有し、電力・リアルタイム性の両面で不利。

## 変更方針
1. **ACK Future の撤廃**
   - `ActorCell` の `pending_create_ack` / `prepare_create_ack` を削除し、Create 完了通知は LifecycleEvent のみにする。
   - `ActorSystem::build_cell_for_spawn` から Future を返さず、セル登録のみを行う。
2. **spawn ハンドシェイクの簡素化**
   - `perform_create_handshake` では SystemMessage::Create enqueue 成否のみ確認し、成功時は即座に `ChildRef` を返す。
   - enqueue 失敗時（メールボックス満杯など）のみロールバックして `SpawnError::invalid_props("create system message delivery failed")` を返す。
3. **失敗観測の経路整理**
   - `pre_start` 失敗・panic は `SystemMessage::Failure` と EventStream (LifecycleEvent::Stopped/Restarted) で通知する。
   - 親アクター/スーパーバイザが必要に応じてアプリケーション層へ応答を返す（明示的な reply-to を利用）。

## テスト戦略
- 既存 ACK 依存テスト（timeout 等）を削除し、代わりに「Create enqueue 後すぐにユーザーメッセージを送っても pre_start 待機が発生しない」シナリオを追加。
- Create enqueue 失敗時のロールバック確認テストは維持しつつ、結果判定から ACK 固有文字列を除去。

## ドキュメント更新
- README の「dispatcher ACK を待機」と記述した箇所を削除し、「ライフサイクル成功は EventStream / Supervisor で観測」「成功可否を知りたければアプリケーションレベルの reply-to を使う」旨を追記。

## 既知の非目標
- 遠隔メッセージングや将来の backpressure 制御における business-level ACK は今回の範囲外。
- Actor 用リクエスト/レスポンス API（`ask` 高級ラッパー）の設計変更も別タスクで扱う。
