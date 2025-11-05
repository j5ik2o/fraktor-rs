# 提案: SystemMessage::Failure で監督失敗を統一

## Why
- 現状の cellactor-rs では子アクターの失敗通知が `SystemState::notify_failure` → 親セル呼び出しという内部 API で散在しており、mailbox 経由の SystemMessage には統合されていない。
- Proto.Actor では `Failure` system message を親に送ることで監督処理とメールボックスが連動し、`EscalateFailure` も共通経路に乗るcitereferences/protoactor-go/actor/messages.goreferences/protoactor-go/actor/actor_context.go。
- Pekko でも `Failed(child, cause, uid)` SystemMessage を介して再起動・停止や StashWhenFailed の制御を行っており、Failure 系のワークフローを mailbox に集約しているcitereferences/pekko/actor/src/main/scala/org/apache/pekko/dispatch/sysmsg/SystemMessage.scala。
- Failure を SystemMessage として正式に定義することで、将来的な failure hooks、監督戦略の差し替え、メトリクス集計、遠隔監督などを Proto.Actor / Pekko と同等に拡張できる。

## What Changes
1. `SystemMessage::Failure` を追加し、PID・原因・再起動統計・失敗メッセージを mailbox で配送する。
2. `ActorCell` / `SystemState` に散在する failure 通知を `SystemMessage::Failure` 経由に差し替え、監督戦略が mailbox 経由で一貫して起動するようにする。
3. Escalation・監督ポリシー・メトリクスを Failure メッセージに紐付け直し、Proto.Actor / Pekko と互換性のあるイベントライフサイクルを保証するテストとドキュメントを更新する。
