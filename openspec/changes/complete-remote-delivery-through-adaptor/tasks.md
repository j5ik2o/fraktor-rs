## 1. 仕様差分の整理

- [x] 1.1 `openspec/specs/remote-core-extension/spec.md` から pivot 前の古い `Remoting` `&mut self` / `&[Address]` requirement を削除し、`RemoteShared` `&self` / `Vec<Address>` contract だけを残す。
- [x] 1.2 `openspec/specs/remote-core-transport-port/spec.md` を更新し、`RemoteTransport::send` が `Box<OutboundEnvelope>` または同等の元 envelope 返却経路を持つ、retry 可能な error を返すことを反映する。
- [x] 1.3 `openspec/specs/remote-adaptor-std-io-worker/spec.md` を更新し、`RemoteShared` event-loop model と衝突する古い `AssociationRegistry` / `StdRemoting` driver requirement を削除する。
- [x] 1.4 watermark の意味を決定し、`openspec/specs/remote-core-association-state-machine/spec.md`、`openspec/specs/remote-core-extension/spec.md`、実装を揃える。
  - 推奨: 内部 high-watermark 観測用に `BackpressureSignal::Notify` を正式化し、`Apply` は実際に user lane を pause する用途に残す。
  - 代替: `Notify` を削除し、`Apply` によって drain helper が low watermark release に到達不能にならないことを証明する。

## 2. outbound TCP 配送

- [x] 2.1 std remote 配送用の絞った outbound payload codec contract を追加する（最小 `Bytes` と `Vec<u8>` の両方）。任意 `AnyMessage` が serialize 可能であるかのように扱わない。
- [x] 2.2 `OutboundEnvelope -> EnvelopePdu` 変換を実装し、recipient path、optional sender path、priority、correlation id high/low fields、payload bytes を保持する。
- [x] 2.3 `TcpRemoteTransport::send` を変更し、transport が running かつ既存 peer writer がある場合は、無条件 `SendFailed` ではなく `WireFrame::Envelope` を enqueue する。
- [x] 2.4 error path を観測可能かつ retry 可能にする。
  - 未起動 -> 元 envelope 付き `NotStarted`
  - peer writer 不在 -> 元 envelope 付き `ConnectionClosed`
  - 未サポート payload / serialization failure -> 元 envelope 付き `SendFailed` または明示的に mapping された transport error
- [x] 2.5 変換成功と未サポート payload 失敗の unit test を追加する。
- [x] 2.6 `RemoteTransport::send` が接続済み peer へ envelope frame を書く TCP integration test を追加する。

## 3. inbound local 配送

- [x] 3.1 現在の `RemoteShared::run` future では event ごとの hook を挟めない場合、core logic を重複させない per-event `RemoteShared` orchestration API を追加または公開する。
- [x] 3.2 必要に応じて `RemotingExtensionInstaller::spawn_run_task` を adapter-owned loop に更新する。1 件の `RemoteEvent` を受け取り、`RemoteShared` に処理させ、inbound envelopes を drain して deliver し、core termination で停止する。
- [x] 3.3 `InboundEnvelope::recipient` を local actor system / provider で解決し、reconstructed `AnyMessage` を local actor ref へ送る adapter delivery bridge を実装する。
- [x] 3.4 delivery が remote write lock の外で行われることを保証する。
- [x] 3.5 delivery failure を actor-core dead-letter / error convention または明示的 adapter error path へ流す。silent drop しない。
- [x] 3.6 サポート対象 payload の inbound envelope frame が local actor mailbox に届く test を追加する。

## 4. connection-loss event の配線

- [x] 4.1 TCP client / server task が `RemoteEvent::ConnectionLost` を emit できるよう、peer authority を識別できる情報を保持する。
- [x] 4.2 association 後の connection close / write failure / decode failure を `ConnectionLost { authority, cause, now_ms }` として emit する。
- [x] 4.3 通常の transport shutdown が誤解を招く connection-loss recovery event を emit しないことを保証する。
- [x] 4.4 connection loss が `Remote::handle_remote_event` に届き、gate / recover behavior を起動する test を追加する。

## 5. cluster remoting event subscription

- [x] 5.1 `subscribe_remoting_events` を修正し、`EventStreamSubscription` が意図した lifetime 中に保持されるか caller に返るようにする。
- [x] 5.2 helper return 後に publish された remoting lifecycle event が cluster topology を更新する regression test を追加する。
- [x] 5.3 provider shutdown / drop 後に subscription が leak しないことを確認する。

## 6. extension installer の config 経路

- [x] 6.1 `ExtensionInstallers` または同等の actor-core extension registry に、caller が保持する shared installer handle を登録できる API を追加する。
- [x] 6.2 `RemotingExtensionInstaller` を `ActorSystemConfig::with_extension_installers` 経由で install しても、caller が同じ handle から `remote()` を取得し、`remote().start()` 後に `spawn_run_task()` / `shutdown_and_join()` を呼べるようにする。
- [x] 6.3 `showcases/std/legacy/remote_lifecycle/main.rs` を修正し、`installer.install(&system)` を削除して `ActorSystemConfig::with_extension_installers` に installer を渡す。
- [x] 6.4 `showcases/std/legacy/tests/remote_lifecycle_surface.rs` または同等の surface test を更新し、remote lifecycle showcase が config install 経路を使い、direct install を含まないことを検証する。
- [x] 6.5 std remote adapter の actor-ref provider installer または builder helper を追加し、`StdRemoteActorRefProvider` を `ActorSystemConfig::with_actor_ref_provider_installer` 経由で登録できるようにする。
- [x] 6.6 `ActorSystem::resolve_actor_ref(remote path)` が config 登録済み `StdRemoteActorRefProvider` を通り、resolved `ActorRef` への tell が `RemoteEvent::OutboundEnqueued` を push する regression test を追加する。`StdRemoteActorRefProvider::actor_ref` の direct unit test だけで済ませない。

## 7. end-to-end 証明

- [x] 7.1 two-node `remote-adaptor-std` integration test を追加する。
  - 両 system に remote installer を `ActorSystemConfig::with_extension_installers` 経由で登録する。
  - 両 system に remote-aware actor-ref provider を `ActorSystemConfig::with_actor_ref_provider_installer` 経由で登録する。
  - remote を start し run task を起動する。
  - 必要な TCP peer connection を確立する。
  - actor-core provider dispatch 経由で remote actor ref を resolve する。
  - サポート対象 payload を送る。
  - remote 側 local actor が payload を受信したことを assert する。
- [x] 7.2 `ClusterApi::get` / `GrainRef` または既存の最も近い cluster remote entry point 経由で、std remote adapter delivery path に到達する cluster 向け integration test を追加する。
- [x] 7.3 サポート対象 payload serialization を test で明示する。serializer registry を実装していない限り、任意 typed message を使わない。

## 8. 検証

- [x] 8.1 `rtk cargo test -p fraktor-actor-core-rs`
- [x] 8.2 `rtk cargo test -p fraktor-remote-core-rs`
- [x] 8.3 `rtk cargo test -p fraktor-remote-adaptor-std-rs`
- [x] 8.4 `rtk cargo test -p fraktor-cluster-adaptor-std-rs`
- [x] 8.5 `rtk cargo test -p fraktor-showcases-std --test remote_lifecycle_surface`
- [x] 8.6 `rtk cargo check --tests`
- [x] 8.7 `rtk ./scripts/ci-check.sh ai all`
