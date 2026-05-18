## 1. Core Redelivery State

- [x] 1.1 `EnvelopePdu` に system priority envelope 用の redelivery sequence metadata を追加する
- [x] 1.2 `remote-core` association の system priority envelope に sequence number / pending window を追加する
- [x] 1.3 `AckPdu` の cumulative ack / nack bitmap を association state に適用する
- [x] 1.4 inbound system sequence tracking と duplicate suppression を追加する
- [x] 1.5 redelivery window、ACK、NACK、duplicate の unit test を `fraktor-remote-core-rs` に追加する

## 2. Watcher State

- [x] 2.1 `WatcherEffect` に remote `Watch` / `Unwatch` system message 送信指示を追加する
- [x] 2.2 `RewatchRemoteTargets` 相当の effect が target と watcher の actor path を保持するよう更新する
- [x] 2.3 `NotifyTerminated` の idempotency と heartbeat 後の再通知許可を unit test で固定する

## 3. Actor-Core Boundary

- [x] 3.1 remote-bound `DeathWatchNotification` を扱えるよう remote watch hook surface を拡張する
- [x] 3.2 hook が remote watch/unwatch/notification を消費した場合に actor-core fallback が走らない test を追加する
- [x] 3.3 inbound remote notification が既存 DeathWatch dedup を通る test を追加する

## 4. Std Provider Wiring

- [x] 4.1 `StdRemoteActorRefProvider` に synthetic remote pid と canonical actor path の registry を追加する
- [x] 4.2 provider installer が actor-core に remote watch hook を登録する
- [x] 4.3 hook が target / watcher pid を actor path へ解決し、watcher task command へ変換する
- [x] 4.4 mapping 解決不能時に hook が `false` を返し、既存 fallback を維持する test を追加する
- [x] 4.5 hook が remote-bound `DeathWatchNotification` を system priority envelope に変換する

## 5. Std Watcher Task And Retry Driver

- [x] 5.1 std watcher task を追加し、`WatcherState` を command queue と monotonic timer で駆動する
- [x] 5.2 `WatcherEffect::SendHeartbeat` を `ControlPdu::Heartbeat` / response handling に接続する
- [x] 5.3 remote watch/unwatch/rewatch effect を system priority envelope に変換する
- [x] 5.4 `NotifyTerminated` を local watcher への `SystemMessage::DeathWatchNotification` に接続する
- [x] 5.5 retry driver が core association の resend / ack effects を実行する

## 6. Integration Verification

- [x] 6.1 two-node TCP test で remote actor 終了が watcher に通知されることを確認する
- [x] 6.2 two-node TCP test で remote `Unwatch` 後の古い通知が user handler に届かないことを確認する
- [x] 6.3 ACK 欠落または NACK を注入し、watch system message が resend で回復することを確認する
- [x] 6.4 `cargo test -p fraktor-actor-core-kernel-rs` を実行する
- [x] 6.5 `cargo test -p fraktor-remote-core-rs` を実行する
- [x] 6.6 `cargo test -p fraktor-remote-adaptor-std-rs` を実行する
- [x] 6.7 `cargo build -p fraktor-remote-core-rs --no-default-features` を実行する
- [x] 6.8 実装完了時に `docs/gap-analysis/remote-gap-analysis.md` の該当 gap を更新する
