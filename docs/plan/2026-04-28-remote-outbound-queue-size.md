# remote outbound queue size 実装計画

## 対象

TAKT `pekko-porting` ワークフロー Phase 2 の advanced Artery settings 残件として、outbound message/control queue size を設定値だけでなく送信キューの実挙動へ接続する。

## 変更予定

| 種別 | ファイル |
|------|---------|
| 変更 | `modules/remote-core/src/core/config/remote_config.rs` |
| 変更 | `modules/remote-core/src/core/association/send_queue.rs` |
| 変更 | `modules/remote-core/src/core/association/offer_outcome.rs` |
| 変更 | `modules/remote-core/src/core/association/base.rs` |
| 変更 | `modules/remote-core/src/core/config/tests.rs` |
| 変更 | `modules/remote-core/src/core/association/tests.rs` |

## 実装方針

`RemoteConfig` に `outbound_message_queue_size` と `outbound_control_queue_size` を追加し、Pekko Artery と同じく 0 を拒否する。`Association::from_config` で設定値を `SendQueue` と deferred buffer の上限へ渡し、overflow は envelope を失わず `AssociationEffect::DiscardEnvelopes` として観測可能にする。

## スコープ外

- inbound restart / remove-quarantined / large-message 系設定
- remote `ActorRef` 実体化
- payload serialization / inbound envelope delivery
- DeathWatch / watcher effects application
