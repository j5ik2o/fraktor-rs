## Context

`remote-gap-analysis.md` の Phase 2 は medium gap を 2 件に絞っている。1 つ目は `RemoteRouterConfig` の `ConsistentHashingPool` が serializer で `NotSerializable` になる点、2 つ目は `RemoteConfig` に保持済みの advanced settings が実際の送受信処理に十分反映されていない点である。

現行コードでは `ConsistentHashingPool::new(nr_of_instances, hash_key_mapper)` が任意クロージャを保持する。これはプロセス外へ安全に表現できないため、`MiscMessageSerializer` は `ConsistentHashingPool` を明示的に拒否している。一方で `ConsistentHashingRoutingLogic` は `ConsistentHashableEnvelope` の明示 `hash_key` を優先するため、この経路だけなら wire 表現できる。

`RemoteConfig` 側では `large_message_destinations`、`outbound_large_message_queue_size`、`inbound_lanes`、`outbound_lanes`、`compression_config` が既に型付きで保持されている。ただし `TcpRemoteTransport::from_config` が確実に適用しているのは主に bind / advertised address / frame size で、large-message 分離や lane 数はまだ送受信処理へ接続されていない。

## Goals / Non-Goals

**Goals:**

- `RemoteRouterConfig` が wire-safe consistent-hashing pool を encode/decode できる。
- 任意クロージャ mapper は `NotSerializable` のまま残し、lossy な代替表現を作らない。
- `large_message_destinations` に一致する user message を通常 user queue と別枠で扱う。
- `outbound_large_message_queue_size` を large-message queue の上限として使う。
- `outbound_lanes` を peer ごとの writer lane 数として反映する。
- `inbound_lanes` を TCP reader から remote event sender への dispatch lane 数として反映する。
- compression 設定は保持のみの契約を維持し、wire 圧縮を暗黙に有効化しない。

**Non-Goals:**

- 任意 `hash_key_mapper` closure の serialization。
- `AnyMessage` payload に対する direct `ConsistentHashable` trait-object dispatch。
- 任意 actor message serialization。
- ACK/NACK redelivery。
- remote DeathWatch / `AddressTerminated` 統合。
- remote deployment daemon。
- compression table advertisement / wire-level compression。
- Pekko Artery wire compatibility。

## Decisions

### Decision 1: consistent-hashing は explicit envelope key だけ wire-safe とする

`ConsistentHashingPool` に wire-safe constructor を追加し、内部 mapper 種別を識別できるようにする。候補名は `ConsistentHashingPool::new_envelope_hash_key(nr_of_instances)` とする。

この pool は次の意味を持つ。

- message が `ConsistentHashableEnvelope` の場合は `hash_key()` を使う。
- envelope でない message の fallback は固定値を返す。
- fallback は任意 payload の安定 hash ではないため、利用者は consistent-hashing 対象 message を明示 envelope で送る必要がある。

```
RemoteRouterConfig
  └─ RemoteRouterPool::ConsistentHashing
       ├─ EnvelopeHashKey     -> serializable
       └─ CustomClosure       -> NotSerializable
```

Rationale:

- 任意クロージャは wire に載せられない。
- `TypeId` や Rust 型名はプロセス間の安定 hash key にできない。
- `ConsistentHashableEnvelope` の `u64` key は既に explicit な wire-safe 値である。

### Decision 2: serializer は pool tag と mapper tag を分ける

`MiscMessageSerializer` の `RemoteRouterConfig` encoding は、既存 pool tag に `ConsistentHashingPool` を追加し、consistent-hashing の場合だけ mapper tag を追加する。

想定 wire layout:

```text
pool_tag
nr_of_instances
router_dispatcher
optional_pool_payload
nodes
```

`optional_pool_payload` は consistent-hashing pool の mapper tag を保持する。既存 `SmallestMailboxPool` / `RoundRobinPool` / `RandomPool` では空の payload として扱う。

Rationale:

- 既存 `RORRC` manifest を維持できる。
- pool 固有 payload を明示すれば、将来の wire-safe mapper 追加時も `RemoteRouterConfig` 全体の意味を壊さない。
- arbitrary closure を encode する余地は作らない。

### Decision 3: large-message は local outbound scheduling のみを変える

`Association::from_config` は large-message destination patterns と large-message queue limit を保持し、`Association::enqueue` が recipient path を見て queue class を決める。

```
OutboundEnvelope
      │
      ▼
Association::enqueue
      │
      ├─ priority == System ─────────────▶ system queue
      ├─ recipient matches large pattern ─▶ large-message queue
      └─ otherwise ──────────────────────▶ user queue

drain order:
  system -> user -> large-message
```

Rationale:

- `EnvelopePdu` の priority wire layout は `System` / `User` のまま維持する。
- large-message は送信側の queue 分離であり、受信側 wire priority を増やす必要はない。
- system message を large-message queue に入れないことで DeathWatch / handshake / control 系を user payload より優先できる。

### Decision 4: outbound lanes は peer connection 内の writer queue として扱う

`TcpClient` は `outbound_lanes` 個の bounded writer queue を持ち、1 つの writer task が lane を公平に drain する。`TcpRemoteTransport::send` は envelope の recipient path、sender path、correlation id から lane index を安定選択する。

```
TcpRemoteTransport::send
      │
      ▼
lane = hash(recipient, sender, correlation) % outbound_lanes
      │
      ▼
TcpClient lane queue[N]
      │
      ▼
single TCP writer task
```

Rationale:

- lane 数を増やしても peer との TCP connection 数を増やさない。
- 1 connection の writer task が最終書き込み順を制御するため、fraktor 独自 frame codec の前提を維持できる。
- bounded queue を lane ごとに分けることで、大きい user payload が全 user traffic を詰まらせにくくなる。

### Decision 5: inbound lanes は reader から remote event sender への dispatch lane とする

TCP reader は decode 済み frame を `inbound_lanes` 個の dispatch lane に振り分ける。同一 association の frame は authority / actor path 由来の key で同じ lane に寄せる。各 lane は `RemoteEvent::InboundFrameReceived` を remote event sender へ送る。

Rationale:

- `Remote` 側の状態更新は引き続き single owner のままにできる。
- TCP reader と event sender の間の backpressure / decode failure handling を lane 単位で観測できる。
- 同一 association の frame を同一 lane に寄せることで、connection 内で発生する不要な reorder を避ける。

### Decision 6: compression はこの change では wire behavior にしない

`RemoteCompressionConfig` は Phase 3 の serializer registry / payload codec 設計の入力として保持する。現在の Phase 2 では compression config を transport / wire codec に接続しない。gap analysis はこの判断に合わせて、compression advertisement / table application を Phase 2 の完了条件から外す。

Rationale:

- 既存 `remote-core-settings` spec が wire-level compression を明示的に非対象としている。
- 任意 actor message serialization が未配置のまま compression table を導入すると、圧縮対象の manifest / actor ref を確定できない。
- 設定保持と wire behavior を混ぜない方が、Phase 3 の serializer registry 境界を明確にできる。

## Risks / Trade-offs

- **Risk: envelope-only consistent hashing が狭すぎる。**  
  任意 mapper を wire に載せるより、明示 hash key を要求する方が安全である。direct `ConsistentHashable` dispatch は別 change で検討する。

- **Risk: large-message queue が wire priority とずれる。**  
  queue 分離は local scheduling の契約であり、wire priority は system/user のままにする。受信側で large-message として扱う機能は本 change に含めない。

- **Risk: outbound lanes が実際の TCP 並列化ではない。**  
  1 connection の writer task に集約するため書き込みは最終的に直列化される。ただし queue 分離による head-of-line blocking 緩和はできる。

- **Risk: compression Phase 2 完了に見えない。**  
  既存 spec と矛盾しないため、compression は「保持のみ」と明記し、gap analysis の Phase 2 項目を large-message / lanes / router serialization に修正する。
