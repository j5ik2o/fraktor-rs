## 実装タスクリスト

### Phase 1: ランタイムの非同期化
- [x] `ActorSystem::spawn_with_parent` から Create ACK future の生成と待機処理を削除し、SystemMessage::Create enqueue 成否のみを結果とする
- [x] `ActorCell` から `pending_create_ack` フィールドおよび `prepare_create_ack` / `notify_create_result` を撤廃し、Create 完了時の副作用を LifecycleEvent へ一本化する

### Phase 2: テスト整備
- [x] 書き換え後の `ActorSystem` / `ActorCell` 単体テストを更新し、Create ACK 依存のケース（busy-spin timeout 等）を削除する
- [x] 新たに fire-and-forget 前提を検証するテスト（Create enqueue 後に即時 user メッセージを送っても pre_start 完了を待たないなど）を追加する

### Phase 3: ドキュメント更新
- [x] README や関連ガイドから「dispatcher ACK を待機する」といった記述を削除し、fire-and-forget で届けられたメッセージはアプリケーション側で応答を組むべきことを明記する
