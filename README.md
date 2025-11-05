# cellactor-rs

cellactor-rs は Akka/Pekko 互換のアクターランタイムを Rust/no_std で実装することを目的とした実験的なプロジェクトです。dispatch や EventStream、SupervisorStrategy などの基盤機能に加えて、DeathWatch 互換の監視 API を強化し、より直感的なアクターモデルを提供します。

## DeathWatch API のハイライト

- `ActorContext::watch/unwatch` で任意のアクターを監視可能。
- `ActorContext::spawn_child_watched` で子アクター生成と DeathWatch 登録を一括で実行。
- 監視対象が停止すると `Actor::on_terminated` が呼び出され、復旧ロジックを Actor 内に閉じ込められる。
- 既に停止したアクターを監視した場合でも、即時に `SystemMessage::Terminated` が通知され、EventStream を経由しない低遅延な挙動を実現。

## クイックスタート

```rust
ctx.watch(child.actor_ref())?; // 監視開始
ctx.unwatch(child.actor_ref())?; // 監視解除

fn on_terminated(
  &mut self,
  ctx: &mut ActorContext<'_>,
  terminated: Pid,
) -> Result<(), ActorError> {
  ctx.log(LogLevel::Info, format!("{:?} stopped", terminated));
  Ok(())
}
```

より詳細なチュートリアルや移行ガイドは `docs/` 配下を参照してください。

## ライフサイクル制御の統一

- アクターの起動と再起動は `SystemMessage::Create` / `SystemMessage::Recreate` として mailbox に投入され、ユーザーメッセージより必ず先に処理されます。
- `ActorSystem::spawn_with_parent` は fire-and-forget で動作し、`SystemMessage::Create` の enqueue が成功した時点で `ChildRef` を返します。`pre_start` の結果は EventStream や Supervisor を通じて観測してください。
- Restart 指示は `SystemMessage::Recreate` を経由して `post_stop` → インスタンス再生成 → `pre_start(LifecycleStage::Restarted)` の順序を保証し、送信に失敗した場合は Stop/Escalate へフォールバックします。
- 子アクターの失敗は `SystemMessage::Failure` として親へ配送され、監督戦略・メトリクス・EventStream が同じ経路を共有します。
