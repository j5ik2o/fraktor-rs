# 実装計画

- [x] 1. DistributedPubSubSettings の core contract を追加する
  - role filter、routing mode、gossip interval、removed entry TTL、max delta elements、no-subscriber behavior を保持できる settings を追加する。
  - unsupported routing mode と invalid max delta elements が typed configuration error として観測できるようにする。
  - settings の unit test で default 値、role filter、routing validation、dead-letter behavior が確認できる。
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
  - _Boundary: DistributedPubSubSettings_

- [x] 2. mediator command と acknowledgement protocol を追加する
  - `Put`、`Remove`、`Subscribe`、`Unsubscribe`、`Publish`、`Send`、`SendToAll`、query command を core protocol として表現する。
  - subscribe / unsubscribe acknowledgement と current topics / subscriber count query result を返せるようにする。
  - invalid topic、invalid path、扱えない payload が validation failure として観測できる。
  - path command は canonical URI を actor-core `ActorPathParser`、absolute / relative selection を `ActorSelectionResolver` 相当で解決し、`ActorPath::to_relative_string()` による address-less relative registry key を使う。address は local owner 判定にだけ使う。
  - command protocol の unit test で command validation、ack、query result の完了状態を確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 3.5_
  - _Boundary: MediatorProtocol_
  - _Depends: 1_

- [x] 3. topic / path registry bucket と tombstone を追加する
  - owner identity、bucket version、path entry、topic subscription entry、removed tombstone を同じ registry bucket で保持できるようにする。
  - local mutation ごとに monotonic version が進み、remove entry が removed TTL まで tombstone として残るようにする。
  - removed member の bucket を delivery candidate から外せる state view を提供する。
  - registry bucket の unit test で put/remove/version/tombstone/prune と entry namespace の分離を確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 4.1, 4.4, 4.5, 5.2_
  - _Boundary: TopicRegistryBucket_
  - _Depends: 1, 2_

- [x] 4. Send / SendToAll path semantics を実装する
  - `Send` が matching path entry のうち1つを settings routing mode に従って選べるようにする。
  - local affinity が local owner entry を優先し、存在しない場合に cluster-wide candidate へ fallback できるようにする。
  - `SendToAll` が matching path entry 全体へ delivery intent を作り、all-but-self では local owner を除外するようにする。
  - path semantics の unit test で canonical relative key、parse failure、one-of、local affinity、all-of、all-but-self、no-subscriber drop/dead-letter intent を確認できる。
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.6_
  - _Boundary: PubSubPathSemantics_
  - _Depends: 2, 3_

- [x] 5. topic publish と mediator delivery intent を registry に接続する
  - topic subscription registry に基づいて `Publish` の delivery intent を生成できるようにする。
  - optional group を registry entry として保持し、topic publish と path `Send` が別 semantics で処理されるようにする。
  - `PubSubBroker` / `ClusterPubSub` / `PubSubApi` から mediator state を利用できる最小接続を追加する。
  - integration test で subscribe、publish、unsubscribe、query result が registry mutation と delivery intent に接続されることを確認できる。
  - _Requirements: 1.3, 1.4, 1.5, 1.6_
  - _Boundary: MediatorProtocol, TopicRegistryBucket_
  - _Depends: 3, 4_

- [x] 6. topic registry status / delta collection を追加する
  - owner version map を持つ registry status と bounded registry delta payload を生成できるようにする。
  - peer status と local bucket version を比較し、max delta elements を超えない version-order chunk を返す。
  - stale delta、unknown owner、non-active member delta が ignored outcome として観測できるようにする。
  - delta collector の unit test で status comparison、chunking、stale apply、unknown owner ignored、tombstone prune を確認できる。
  - _Requirements: 2.5, 4.2, 4.3, 4.4, 4.5, 4.6_
  - _Boundary: TopicRegistryDeltaCollector_
  - _Depends: 3_

- [x] 7. membership / gossip integration boundary を接続する
  - membership current state から role filter と active member status に基づく mediator peer set を更新できるようにする。
  - registry gossip tick で pubsub registry status / delta payload を生成し、`PubSubGossipHandoff` 経由で `GossipPayloadKind` の logical pubsub status / delta kind として渡せる payload contract を追加する。
  - pubsub payload が membership `GossipOutbound`、wire transport、byte tag assignment、gossip envelope framing、heartbeat scheduling、reachability merge、downing decision を所有しないことを boundary test で確認できる。
  - gossip integration test で member removed/downed/left により bucket が delivery candidate から外れることを確認できる。
  - _Requirements: 2.2, 5.1, 5.2, 5.3, 5.4, 5.5_
  - _Boundary: MediatorPeers, TopicRegistryGossipPayload, PubSubGossipHandoff, ScopeGuard_
  - _Depends: 6_

- [x] 8. std delivery bridge と gap analysis evidence を更新する
  - std adaptor が core delivery intent を実行し、target selection と mediator protocol semantics を再計算しないようにする。
  - actor serialization は existing extension 利用に留め、cluster message serializer framework をこの実装に含めない。
  - `cluster-core-kernel` の targeted tests と no_std check で core に Tokio / std I/O が混入していないことを確認する。
  - `docs/gap-analysis/cluster-gap-analysis.md` に `DistributedPubSubMediator` protocol、settings、path semantics、topic registry gossip / delta collection の evidence だけを反映する。
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
  - _Boundary: StdDeliveryBridge, ScopeGuard, GapAnalysisUpdate_
  - _Depends: 7_
