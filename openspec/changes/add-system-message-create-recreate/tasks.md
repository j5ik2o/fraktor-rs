## 実装タスクリスト

### Phase 1: SystemMessage拡張
- [ ] `modules/actor-core/src/messaging/system_message.rs` に `Create` / `Recreate` variant を追加
- [ ] 単体テスト追加（variant round-trip）

### Phase 2: ActorCellのメッセージ処理
- [ ] `ActorCellGeneric::invoke_system_message` に `Create` / `Recreate` 分岐を追加
- [ ] `handle_create` / `handle_recreate` ヘルパーを実装し、`run_pre_start` / `post_stop` 呼び出しを移行
- [ ] 既存の `pre_start` / `restart` 呼び出し箇所を新SystemMessage経由にリファクタ

### Phase 3: SystemState / Supervisor 統合
- [ ] spawn フロー（`ActorSystem::spawn_with_parent` など）で `SystemMessage::Create` を enqueue
- [ ] dispatcher からの ACK (oneshot など) を待ち、`pre_start` 成否を spawn 呼び出しへ伝搬する
- [ ] 再起動フロー（`SystemState::handle_failure` 等）で `SystemMessage::Recreate` を enqueue
- [ ] `SystemMessage::Recreate` 送信失敗時に `SystemMessage::Stop` へフォールバックし、Supervisor へ Escalate する処理を実装
- [ ] 子プロセスの再登録や統計クリア処理が新経路でも動作することを確認

### Phase 4: テスト / ドキュメント
- [ ] `ActorCell` / `SystemState` 単体テスト更新（Create/Recreate 経路）
- [ ] 監督・再起動系の結合テストで `post_stop` → 再生成 → `pre_start` (Restart) の順序が SystemMessage 経由で実行されることを検証
- [ ] spawn 直後の通常メッセージが Create 完了前に処理されないことを確認するテストを追加
- [ ] LifecycleEvent (Started/Restarted) が EventStream に既存通り publish されることを検証
- [ ] ドキュメント（README or guides）に SystemMessage 化の背景を記載

### Phase 5: エラーハンドリングと同期保証
- [ ] Create 送信失敗時に `rollback_spawn` が呼ばれることを検証する結合テスト
- [ ] dispatcher ACK 失敗・タイムアウト時のフォールバック動作をテスト
- [ ] Recreate 送信失敗時に Stop/Escalate へ切り替わることを確認するテスト
