# 変更履歴

## Unreleased

### Breaking Changes

- **動的SupervisorStrategy取得**: `Props::with_supervisor()` と `Props::supervisor()` を削除し、`Actor::supervisor_strategy(&mut self, &mut ActorContext) -> SupervisorStrategy` メソッドを追加しました。
  - これにより、アクターは内部状態に基づいて動的に監督戦略を決定できるようになりました（Pekko Classicの `Actor#supervisorStrategy` と互換）。
  - `SupervisorStrategy` と `SupervisorOptions` から `Copy` トレイトを削除しました。
  - `ActorCell` から固定の `supervisor` フィールドを削除しました。
  - 移行方法: `Props::from_fn(MyActor::new).with_supervisor(strategy)` の代わりに、`MyActor` の `supervisor_strategy()` メソッドをオーバーライドしてください。

### Features

- SystemMessage に `Watch/Unwatch/Terminated` variant を追加し、DeathWatch を実装。
- `ActorContext::watch/unwatch/spawn_child_watched` と `Actor::on_terminated` を導入。
- ActorCell に監視者リストと Terminated 通知処理を追加。
- actor-core/actor-std 両環境向けの DeathWatch 統合テストとサンプルを追加。
- typed API に `Behaviors::supervise` を追加し、宣言的に子アクターの監督戦略を設定できるようにしました。`modules/actor-std/examples/behaviors_supervise_typed_std` で使用例を追加し、typed ランタイムが `SupervisorStrategy` を動的に参照するテストを追加しました。
