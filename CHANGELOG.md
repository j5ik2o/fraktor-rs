# 変更履歴

## Unreleased

- SystemMessage に `Watch/Unwatch/Terminated` variant を追加し、DeathWatch を実装。
- `ActorContext::watch/unwatch/spawn_child_watched` と `Actor::on_terminated` を導入。
- ActorCell に監視者リストと Terminated 通知処理を追加。
- actor-core/actor-std 両環境向けの DeathWatch 統合テストとサンプルを追加。
