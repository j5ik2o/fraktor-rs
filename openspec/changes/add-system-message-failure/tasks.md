## 実装タスクリスト

### Phase 1: 仕様/型拡張
- [ ] `SystemMessage` enum に `Failure` variant を追加し、`AnyMessage` round-trip テストを用意する
- [ ] `FailurePayload`（PID、`Arc<ActorErrorReason>`, `RestartStatistics`, `Option<AnyMessage>`, `timestamp`）を定義し、所有権/ライフタイム設計をドキュメント化する

### Phase 2: ActorCell / Supervisor 連携
- [ ] `ActorCellGeneric::invoke_system_message` で Failure を処理し、監督戦略や `handle_child_failure` を呼び出すエントリポイントを mailbox 側に集約する
- [ ] `EscalateFailure` / `SystemState::notify_failure` など既存の失敗連絡経路を `SystemMessage::Failure` の enqueue に置き換える
- [ ] Failure が StashWhenFailed に相当する動作（ユーザーメッセージを一時停止し再開する）を提供するため、mailbox suspend/resume のタイミングを Failure 経路に組み込む

### Phase 3: メトリクス / エラーフロー
- [ ] Failure enqueue 時・監督結果完了時にメトリクスを publish する hook を追加する（`failure_total`, `failure_restart_total` など）
- [ ] Failure 送信が失敗した場合のフォールバック（Mailbox full / 親不在 / panic）を実装し、ログ・EventStream へ通知する
- [ ] Restart/Stop/Escalate が完了したタイミングで EventStream にシーケンスイベントを publish する
- [ ] Proto.Actor / Pekko 相当の再起動戦略（one-for-one/all-for-one）が Failure 経路でも正しく作動することを検証するテストを ActorCell / SystemState 等に追加する

### Phase 4: ドキュメント / サンプル
- [ ] README やガイドに Failure SystemMessage 追加の背景と利用パターンを追記
- [ ] 監督のベンチマーク・サンプル（例: actor-std）で Failure を観察できるログを追加し、挙動を確認する
