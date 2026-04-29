# remote remove-quarantined-association-after 実装計画

## 目的

Phase 2 の advanced Artery settings 残件から、実動作に接続できる `remove-quarantined-association-after` を実装する。

## 対象

| 領域 | 変更対象 |
|------|----------|
| remote-core config | `modules/remote-core/src/core/config/remote_config.rs`, `modules/remote-core/src/core/config/tests.rs` |
| remote-core association | `modules/remote-core/src/core/association/base.rs`, `modules/remote-core/src/core/association/association_state.rs`, `modules/remote-core/src/core/association/tests.rs` |
| remote-adaptor-std registry | `modules/remote-adaptor-std/src/std/association/association_registry.rs`, `modules/remote-adaptor-std/src/std/association/tests.rs` |

## 実装方針

1. `RemoteConfig` に `remove_quarantined_association_after: Duration` を追加する。既定値は Pekko と同じ 1 hour とし、builder は zero duration を拒否する。
2. `Association::from_config` で設定値を受け取り、`quarantine(reason, now_ms)` 時に削除可能時刻を `AssociationState::Quarantined { resume_at }` へ設定する。
3. `Duration` から monotonic millis への変換は saturating にし、非ゼロ duration が 0ms に丸め込まれないよう 1ms を下限にする。
4. `Association::is_quarantine_removal_due(now_ms)` を追加し、quarantined かつ期限到来の場合のみ true を返す。
5. `AssociationRegistry::remove_quarantined_due(now_ms)` を追加し、期限到来した association の key を収集してから削除し、削除した `UniqueAddress` を返す。

## スコープ外

- 自動 cleanup タスク化
- large-message settings
- inbound restart settings
- remote ActorRef 実体化
- inbound envelope delivery
- DeathWatch / watcher effects application
