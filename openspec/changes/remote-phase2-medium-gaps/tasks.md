## Phase 1: 境界確認

- [ ] 1.1 `docs/gap-analysis/remote-gap-analysis.md` の Phase 2 / Phase 3 境界を確認する
- [ ] 1.2 `MiscMessageSerializer` の `RemoteRouterConfig` encoding / decoding と既存 tests を確認する
- [ ] 1.3 `ConsistentHashingPool` / `ConsistentHashingRoutingLogic` / `ConsistentHashableEnvelope` の現在の hash key precedence を確認する
- [ ] 1.4 `RemoteConfig` の large-message / lanes / compression accessor と既存 settings spec を確認する
- [ ] 1.5 `Association::from_config` / `SendQueue` / `TcpRemoteTransport::from_config` の設定適用箇所を確認する

## Phase 2: wire-safe consistent-hashing pool

- [ ] 2.1 `ConsistentHashingPool` に wire-safe envelope-hash-key constructor を追加する
- [ ] 2.2 `ConsistentHashingPool` が mapper 種別を返せる crate-private query を追加する
- [ ] 2.3 `RemoteRouterPool::ConsistentHashing` から mapper 種別を serializer が判定できるようにする
- [ ] 2.4 arbitrary closure mapper は `NotSerializable` を返す既存挙動を維持する
- [ ] 2.5 envelope-hash-key consistent-hashing pool の router creation が既存 routing logic と同じ precedence を保つことをテストする

## Phase 3: RemoteRouterConfig serialization

- [ ] 3.1 `MiscMessageSerializer` の `RemoteRouterConfig` wire format に consistent-hashing pool tag を追加する
- [ ] 3.2 consistent-hashing pool 用 mapper tag を追加し、envelope-hash-key mapper だけを encode する
- [ ] 3.3 decode 時に envelope-hash-key consistent-hashing pool を復元する
- [ ] 3.4 unknown pool tag / unknown mapper tag / malformed payload を `InvalidFormat` として拒否する
- [ ] 3.5 `RemoteRouterConfig` round-trip tests に consistent-hashing pool を追加する
- [ ] 3.6 arbitrary closure mapper が `NotSerializable` になる regression test を維持する

## Phase 4: large-message queue

- [ ] 4.1 `SendQueue` に large-message user lane と capacity を追加する
- [ ] 4.2 `SendQueue::with_limits` または新しい constructor で system / user / large-message capacity を設定できるようにする
- [ ] 4.3 `Association::from_config` が `outbound_large_message_queue_size` と `large_message_destinations` を保持する
- [ ] 4.4 `Association::enqueue` が user message の recipient path を pattern match し、large-message queue へ振り分ける
- [ ] 4.5 system priority は large-message pattern に一致しても system queue へ入ることをテストする
- [ ] 4.6 drain order が `system -> user -> large-message` であることをテストする
- [ ] 4.7 large-message queue full 時は元 envelope を含む `QueueFull` になることをテストする

## Phase 5: outbound lanes

- [ ] 5.1 `TcpRemoteTransport::from_config` が `outbound_lanes` を `TcpClientConnectOptions` へ渡す
- [ ] 5.2 `TcpClient` が lane 数分の bounded writer queue を持てるようにする
- [ ] 5.3 `TcpClient::send` が lane key から stable lane を選び、該当 lane queue へ enqueue する
- [ ] 5.4 writer task が lane queues を starvation なく drain する
- [ ] 5.5 lane queue full は `TransportError::Backpressure` として返す
- [ ] 5.6 `outbound_lanes = 1` では既存と同等の配送順になることをテストする
- [ ] 5.7 `outbound_lanes > 1` で異なる lane が選ばれる payload を test-observable にする

## Phase 6: inbound lanes

- [ ] 6.1 `TcpRemoteTransport::from_config` が `inbound_lanes` を inbound dispatch 構成へ渡す
- [ ] 6.2 inbound frame event channel を lane 数分に分割する
- [ ] 6.3 TCP reader が authority / actor path 由来の stable key で inbound lane を選ぶ
- [ ] 6.4 各 inbound lane が `RemoteEvent::InboundFrameReceived` を remote event sender へ送る
- [ ] 6.5 same association の frame が同じ inbound lane に入ることをテストする
- [ ] 6.6 lane sender failure が log または error path で観測できることをテストする

## Phase 7: compression boundary / docs

- [ ] 7.1 `RemoteCompressionConfig` はこの change で wire codec / TCP framing に接続しないことを確認する
- [ ] 7.2 compression advertisement / table application を Phase 3 serializer registry 側の future item として design comment に残す
- [ ] 7.3 `docs/gap-analysis/remote-gap-analysis.md` を更新し、Phase 2 完了条件から compression wire behavior を外す
- [ ] 7.4 Phase 2 完了後の残 gap が Phase 3 hard gap へ集中していることを summary に反映する

## Phase 8: verification

- [ ] 8.1 `cargo test -p fraktor-actor-core-kernel-rs misc_message_serializer` を実行する
- [ ] 8.2 `cargo test -p fraktor-actor-core-kernel-rs remote_router_config` を実行する
- [ ] 8.3 `cargo test -p fraktor-remote-core-rs association` を実行する
- [ ] 8.4 `cargo test -p fraktor-remote-adaptor-std-rs transport` を実行する
- [ ] 8.5 `cargo test -p fraktor-remote-adaptor-std-rs two_node_actor_system_delivery` を実行する
- [ ] 8.6 `openspec validate remote-phase2-medium-gaps --strict` を実行する
- [ ] 8.7 実装完了時に `./scripts/ci-check.sh ai all` を実行する
