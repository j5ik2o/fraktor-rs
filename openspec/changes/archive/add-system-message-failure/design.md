# 設計案: SystemMessage::Failure 導入

## 目標
- 失敗通知を mailbox で扱える `SystemMessage::Failure` に統一し、Proto.Actor / Pekko 相当の監督フローを実現する。
- Failure 経路に PID・原因・再起動統計・直近ユーザーメッセージを含めて、監督戦略やメトリクスが情報を失わずに処理できるようにする。

## 参照実装
- Proto.Actor: `Failure` system message を親へ送信し、`EscalateFailure` が mailbox suspend → parent failure deliver → supervisor decisionという流れを形成している。citereferences/protoactor-go/actor/messages.goreferences/protoactor-go/actor/actor_context.go
- Pekko: `Failed(child, cause, uid)` が `SystemMessage` として ActorCell に入ることで StashWhenFailed / StashWhenWaitingForChildren の制御が成立する。citereferences/pekko/actor/src/main/scala/org/apache/pekko/dispatch/sysmsg/SystemMessage.scala

## 主要コンポーネント
1. **SystemMessage::Failure**
   - `FailurePayload` 構造体（`child_pid: Pid`, `cause: Arc<ActorErrorReason>`, `restart_stats: RestartStatistics`, `last_message: Option<AnyMessageGeneric<TB>>`, `timestamp: Duration`）を定義し、`SystemMessage::Failure(FailurePayload)` として運搬。
   - `AnyMessageGeneric` への round-trip テストに加えて、`FailurePayload` の `Arc` / `Option` を使った所有権モデルを明記し、処理完了後に自動でドロップされることを保証。
2. **ActorCell 受信ロジック**
   - `invoke_system_message` に Failure 分岐を追加し、`handle_failure_message` を介して既存の `handle_child_failure` / `supervisor.handle_failure` を呼び出す。
   - Failure 受信時はユーザーメッセージ処理を一時停止（mailbox.suspend）、監督指示（Restart/Stop/Escalate）完了後に再開。
3. **Failure 送信元の統一**
   - `SystemState::notify_failure` / `ActorSystemGeneric::state().notify_failure` など、現在ダイレクトに親へ飛ぶ経路を `send_system_message(pid.parent, SystemMessage::Failure(...))` に変更。
   - Failure enqueue が失敗した場合のフォールバック: (a) Mailbox Full → 親を強制停止し guardian へ Escalate、(b) 親不在 → ルート guardian が Failure を処理、(c) 送信中 panic → システムを graceful shutdown。このフローをコード化し、ログ/メトリクスへ出力。
   - Escalation やルート監督 (`parent == None`) の場合は `SystemState` が直接 Failure を処理してデフォルト監督を起動。
4. **メトリクス / イベント**
   - Failure enqueue 時に `failure_total{pid,cause}` をインクリメントし、`failure_inflight` ゲージを増加。監督結果（Restart/Stop/Escalate）が決まった時点で減算し、`failure_restart_total` など結果別カウンタへ加算。
   - EventStream には (1) Failure 生成イベント、(2) 監督結果イベント（Restart/Stop/Escalate）を publish し、シーケンスを追跡できるようにする。
   - メトリクス/イベントの処理経路を Phase 3 のタスクで実装し、テストで検証する。

## 開発ステップ
1. 型とテストの追加。
2. ActorCell/SystemState の failure 経路を SystemMessage 化。
3. メトリクス・ログ・event-stream の統合。
4. ドキュメント/サンプル更新。

## リスク
- Failure メッセージ導入に伴うメモリコピー増加（`AnyMessage` を内包）。→ `ArcShared` / `Box<dyn Any>` を使い、Clone コストを抑える。
- 既存の failure テストが SystemState ダイレクト通知前提であるため、大規模なテスト更新が必要。
