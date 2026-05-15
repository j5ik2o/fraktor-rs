## Why

remote の残ギャップは、Pekko `RemoteWatcher` が remote node failure 時に `AddressTerminated` を publish する契約が actor-core / std remote stack に存在しない点に集中している。現状は remote target ごとの DeathWatch 通知は行えるが、node-level failure を event stream で観測し、remote deployment watcher や daemon lifecycle が同じ failure signal を共有する境界がない。

## What Changes

- actor-core event stream に remote address termination を表す event と classifier key を追加する。
- remote watcher state が remote node unavailable 判定を actor-level termination だけでなく address-level termination effect として表現できるようにする。
- std watcher task が address termination effect を actor-core event stream へ publish する。
- inbound remote DeathWatch 通知と address termination の責務境界を明確化し、node failure と actor termination を混同しない。
- remote deployment watcher / daemon 側が address termination event を利用して remote-created child の cleanup / failure propagation を実装できる契約を追加する。

## Capabilities

### New Capabilities

- `remote-address-terminated-integration`: remote node failure を actor-core event stream の address-terminated topic 相当へ接続し、std remote watcher と remote deployment lifecycle が共有できる統合契約を定義する。

### Modified Capabilities

- `pekko-eventstream-subchannel`: `AddressTerminated` event variant と classifier key を event stream subchannel contract に追加する。
- `remote-core-watcher-state`: remote node unavailable 判定時に address-level termination effect を返す契約を追加する。
- `actor-core-remote-deathwatch`: address-level termination と actor-level `DeathWatchNotification` の境界を明確化する。

## Impact

- `modules/actor-core-kernel/src/event/stream/`
- `modules/actor-core-kernel/src/system/`
- `modules/remote-core/src/watcher/`
- `modules/remote-adaptor-std/src/watcher.rs`
- `modules/remote-adaptor-std/src/deployment/`
- `modules/remote-adaptor-std/src/extension_installer/`
- `modules/remote-adaptor-std/tests/`
- `docs/gap-analysis/remote-gap-analysis.md`

`actor-core-kernel` と `remote-core` は no_std を維持する。std task、timer、event publication、remote deployment cleanup は `remote-adaptor-std` 側に閉じる。
