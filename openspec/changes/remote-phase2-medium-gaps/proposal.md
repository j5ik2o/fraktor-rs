## Why

`docs/gap-analysis/remote-gap-analysis.md` の Phase 2 には、medium gap として次の 2 件が残っている。

- `consistent-hashing pool remote router serialization`
- `advanced Artery settings` の実処理への反映

remote は byte payload の二ノード配送まで到達しているが、`RemoteRouterConfig` は `ConsistentHashingPool` を wire に載せられず、`RemoteConfig` に保持済みの large-message / lane / compression 設定のうち一部は送受信処理へ反映されていない。このまま Phase 3 の任意 actor message serialization や DeathWatch に進むと、router / queue / transport worker の境界が未整理なまま hard gap に入る。

## What Changes

### 1. consistent-hashing pool remote router serialization

`RemoteRouterConfig` の serializer は、wire-safe な consistent-hashing pool だけを encode/decode できるようにする。

- `ConsistentHashableEnvelope` の明示 `hash_key` を使う built-in mapper を wire 表現の対象にする。
- 任意クロージャの `hash_key_mapper` は引き続き wire 非対応とし、`NotSerializable` を返す。
- `RemoteRouterConfig` の manifest は既存の `RORRC` を維持する。

### 2. large-message / lane 設定の実処理反映

`RemoteConfig` に保持済みの設定を、実際の送受信経路に接続する。

- `large_message_destinations` に一致する user message を通常 user queue から分離し、`outbound_large_message_queue_size` を専用 queue 上限として使う。
- `outbound_lanes` を per-peer outbound writer lane 数として使い、recipient / sender / correlation id から安定した lane を選ぶ。
- `inbound_lanes` を TCP reader から `RemoteEvent` sender へ渡す dispatch lane 数として使い、同一 association の frame は同じ lane へ寄せる。

### 3. compression 設定の扱いを固定する

compression はこの change では wire 圧縮として有効化しない。既存 `remote-core-settings` spec は「compression settings は保持のみ、wire encoding / TCP framing / compression table は変更しない」と定めているため、本 change ではその境界を維持する。実際の compression advertisement / table application は、Phase 3 の serializer registry backed arbitrary user payload serialization と合わせて別 change で扱う。

## Capabilities

### New Capabilities

- `actor-core-remote-router-serialization`
  - `RemoteRouterConfig` が wire-safe consistent-hashing pool を round-trip できる。
  - arbitrary closure mapper は wire 非対応のまま fail-fast する。

### Modified Capabilities

- `remote-core-association-state-machine`
  - `Association` / `SendQueue` が large-message user lane を持ち、system / user / large-message の取り出し順を定義する。
- `remote-core-settings`
  - `RemoteConfig` の advanced settings は、保持だけでなく実処理へ反映される項目と保持のみの項目を区別する。
- `remote-adaptor-std-tcp-transport`
  - `TcpRemoteTransport::from_config` が inbound / outbound lane 数を送受信処理へ反映する。

## Impact

影響コード:

- `modules/actor-core-kernel/src/routing/consistent_hashing_pool.rs`
- `modules/actor-core-kernel/src/routing/remote_router_pool.rs`
- `modules/actor-core-kernel/src/serialization/builtin/misc_message_serializer.rs`
- `modules/remote-core/src/association/send_queue.rs`
- `modules/remote-core/src/association/base.rs`
- `modules/remote-adaptor-std/src/transport/tcp/`
- `docs/gap-analysis/remote-gap-analysis.md`

影響 spec:

- `openspec/specs/remote-core-association-state-machine/spec.md`
- `openspec/specs/remote-core-settings/spec.md`
- `openspec/specs/remote-adaptor-std-tcp-transport/spec.md`
- 新規 `actor-core-remote-router-serialization`

非対象:

- 任意クロージャの serialization
- direct trait-object `ConsistentHashable` dispatch
- serializer registry backed arbitrary user payload serialization
- ACK/NACK redelivery
- remote DeathWatch
- remote deployment
- wire-level compression / compression table advertisement
- Pekko Artery byte compatibility
