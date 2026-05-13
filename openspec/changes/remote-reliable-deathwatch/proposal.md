## Why

現行 remote は remote actor ref の配送と serialized payload delivery までは接続済みだが、remote `watch` / `unwatch` は validation で止まり、remote actor の終了が watcher に届かない。ACK/NACK PDU も観測だけで再送状態に反映されないため、DeathWatch 系 system message が一時的に欠落すると回復できない。

## What Changes

- actor-core の remote watch hook を remote `Watch` / `Unwatch` / `DeathWatchNotification` の配送境界として使えるようにする。
- std remote provider が remote pid と canonical actor path の対応を保持し、actor-core から来た remote watch/unwatch を watcher task へ渡す。
- `remote-core` の association に system priority envelope 用の ACK/NACK redelivery state を追加する。
- std 側に retry driver を追加し、ACK/NACK に基づいて remote DeathWatch 系 system message を再送する。
- std watcher task が `WatcherState` を駆動し、heartbeat、rewatch、terminated notification、quarantine notification の effect を actor-core / remote outbound lane へ適用する。
- `SystemMessage::{Watch, Unwatch, DeathWatchNotification}` は wire 上の local `Pid` 値を信頼せず、envelope の actor path metadata から受信側の pid へ解決してから actor-core へ渡す。
- `FlushBeforeDeathWatchNotification` は本 change では実装しない。DeathWatch 前 flush は `remote-graceful-flush` で扱う。

## Capabilities

### New Capabilities

- `actor-core-remote-deathwatch`: actor-core と remote adaptor の間で remote DeathWatch system message を受け渡す契約。

### Modified Capabilities

- `remote-core-association-state-machine`: system priority envelope の ACK/NACK redelivery state を追加する。
- `remote-core-watcher-state`: `WatcherState` の watch/unwatch/rewatch/terminated effects を remote DeathWatch 用に十分な情報を持つ形へ拡張する。
- `remote-adaptor-std-provider-dispatch`: remote pid/path mapping と remote watch hook 登録を provider dispatch の責務に追加する。
- `remote-adaptor-std-io-worker`: std watcher task と retry driver が core state / actor-core DeathWatch を接続する契約を追加する。

## Impact

- `modules/actor-core-kernel/src/system/remote/` の remote watch hook surface。
- `modules/actor-core-kernel/src/system/state/system_state_shared.rs` の remote-bound system message 分岐。
- `modules/remote-core/src/association/` の system message redelivery state。
- `modules/remote-core/src/watcher/` の command / effect / state。
- `modules/remote-adaptor-std/src/provider/` の remote pid/path registry と hook wiring。
- `modules/remote-adaptor-std/src/association/` と `modules/remote-adaptor-std/src/transport/tcp/` の retry driver / ACK processing。
- `modules/remote-adaptor-std` の two-node integration tests。
