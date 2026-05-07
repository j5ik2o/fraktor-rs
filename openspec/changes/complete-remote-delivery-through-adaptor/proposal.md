## 背景

`pivot-remote-driving-to-core` により、remoting の駆動源は `remote-core` 側へ移った。adapter は `RemoteEvent` を push し、`RemoteShared::run` が association の状態遷移を所有する。前 change は意図的に remote 側契約までで完了しており、end-to-end 配送経路は未完了のまま残っている。

残っている課題は具体的に次の通り。

- `TcpRemoteTransport::send` はまだ `TransportError::SendFailed` を返すだけで、`Remote::handle_remote_event(OutboundEnqueued)` が user envelope を TCP wire へ出せない。
- inbound envelope frame は decode され `Remote` 内に buffer されるが、その `InboundEnvelope` を drain して local actor system へ配送する std adapter worker がない。
- `RemoteEvent::ConnectionLost` と core 側 handler は存在するが、TCP adapter が client / server connection failure からこの event を emit していない。
- `subscribe_remoting_events` は `EventStreamSubscription` を即座に drop しており、cluster topology auto-detection が provider lifetime 中に購読されない。
- live OpenSpec spec には、`Remoting` の `&mut self` method や `RemoteTransport::send -> Result<(), TransportError>` など、pivot 前の契約が残っている。
- watermark backpressure の意味が不整合である。spec は `BackpressureSignal::Apply` / `Release` のみを要求しているが、現在の実装は同じ queue の drain を止めないために `Notify` を追加している。
- `showcases/std/legacy/remote_lifecycle/main.rs` は `RemotingExtensionInstaller` を `ActorSystem` 作成後に直接 `install(&system)` しており、`ActorSystemConfig::with_extension_installers` による bootstrap-time install 経路を使っていない。

この change では std adapter の配送経路を完成させ、以後の remoting 作業が古い spec ではなく pivot 後の architecture から始められるように live spec も整理する。

## 変更内容

### 1. `TcpRemoteTransport::send` を実際の wire enqueue にする

`RemoteTransport::send(OutboundEnvelope)` は同期かつ bounded な API のまま維持する。std TCP 実装は running 中に unconditional `SendFailed` を返してはならない。`OutboundEnvelope` を `WireFrame::Envelope(EnvelopePdu)` に変換し、既存の per-peer TCP writer に enqueue する。

transport port は同期 API なので、この change では `RemoteTransport::send` の中で `TcpStream::connect` を await しない。同期 send が成功するには、adapter が事前に async connection establishment を済ませておく必要がある。既存の `connect_peer` は維持・隠蔽・worker 化のいずれでもよいが、契約は次の通り。

- peer writer が存在する場合、`send` は envelope frame を enqueue して `Ok(())` を返す。
- peer writer が存在しない場合、`send` は元 envelope とともに `ConnectionClosed` を返し、`Remote` が clone なしで re-enqueue できるようにする。
- serialization に失敗した場合、`send` は元 envelope とともに `SendFailed` を返す。
- send error は無言で握りつぶさない。

payload serialization は明示的な契約にする。最初の実装は `bytes::Bytes` / `Vec<u8>` payload のような小さい std adapter codec をサポートし、未サポート payload は観測可能な失敗として拒否してよい。任意の typed `AnyMessage` を暗黙に serialize してはならない。より広い serializer registry が必要なら、この change 内で設計し、cluster / grain e2e scenario が依存する前に test で固定する。

### 2. inbound local-delivery worker を追加する

`Remote::handle_remote_event(InboundFrameReceived)` は envelope frame を decode し、`InboundEnvelope` を buffer できる。std adapter は core event processing 後に `RemoteShared::drain_inbound_envelopes()` を呼び、各 envelope を local actor system / provider へ配送する bridge を持つ必要がある。

この bridge は `remote-adaptor-std` に属する。`remote-core` は既存の `InboundEnvelope` data model を超えて actor-system delivery mechanics を知ってはならない。

run task が orchestration point を所有する。event が `RemoteShared::run` または同等の shared event-step API で処理された直後、adapter は buffer 済み inbound envelopes を drain して配送する。現在の `RemoteShared::run` future がこの hook を挟むには不透明すぎる場合、raw `SharedLock<Remote>` を露出せず、association logic を adapter に重複させない小さい core API を追加する。

### 3. TCP runtime から connection-loss event を emit する

TCP client / server task は、authority が識別できる peer connection の close / failure を remote event channel へ通知する。emit される `RemoteEvent::ConnectionLost { authority, cause, now_ms }` は `Remote::handle_remote_event` により処理され、association の gate / recover に接続される。

これは新しい driver layer ではない。adapter I/O state を既存 core event loop へ報告するだけである。

### 4. actor-core と cluster entry point 経由で remote delivery を証明する

孤立した部品テストだけではなく、実際の経路を通す test を追加する。

```text
ActorSystem::resolve_actor_ref(remote path)
  -> StdRemoteActorRefProvider
  -> RemoteActorRefSender::send
  -> RemoteEvent::OutboundEnqueued
  -> RemoteShared::run / Remote::handle_remote_event
  -> TcpRemoteTransport::send
  -> peer inbound I/O worker
  -> RemoteEvent::InboundFrameReceived
  -> Remote::handle_remote_event
  -> inbound delivery worker
  -> local actor mailbox
```

remote-adaptor test は、選択した serializer contract でサポートされる payload の two-node round trip を検証する。cluster-adaptor test は、`ClusterApi::get` / `GrainRef` または既存の最も近い cluster remote entry point が actor-core provider resolution 経由で remote actor ref を取得し、std remote 配送経路に到達することを証明する。

### 5. cluster remoting event subscription の lifetime を修正する

`subscribe_remoting_events` は返された `EventStreamSubscription` を、`LocalClusterProviderShared` が remoting topology update を必要とする期間保持しなければならない。handle を即 drop すると unsubscribe される。subscription は provider state に保存するか、caller が保持できるように返す。test では、helper return 後に publish された event が topology に反映されることを証明する。

### 6. remote extension installer を config install 経路へ統一する

remote extension の install は application `main` から `installer.install(&system)` を直接呼ぶ形にしてはならない。`RemotingExtensionInstaller` は `ExtensionInstallers` に登録し、`ActorSystemConfig::with_extension_installers` 経由で `ActorSystem::create_with_config` 中に install される必要がある。

ただし `RemotingExtensionInstaller` は stateful installer であり、install 後に caller が `remote()` / `spawn_run_task()` / `shutdown_and_join()` を呼ぶ必要がある。そのため actor-core の extension installer registry は、caller が同じ shared handle を保持したまま config に登録できる API を提供しなければならない。既存の値消費 API だけで caller から installer handle が失われる場合は、shared installer 登録 API または同等の adapter helper を追加する。

`showcases/std/legacy/remote_lifecycle/main.rs` はこの正規経路を示す例に修正する。showcase は `ExtensionInstallers` を作り、`ActorSystemConfig::new(...).with_extension_installers(installers)` を `ActorSystem::create_with_config` に渡す。`installer.install(&system)` は低レベル unit test 以外の user-facing code から除去する。

### 7. live OpenSpec spec を post-pivot 実装へ揃える

古い live specs を整理する。

- `remote-core-extension`: 古い `Remoting` `&mut self` / slice return requirement を削除し、`RemoteShared` `&self` / `Vec<Address>` contract に統一する。
- `remote-core-transport-port`: 元の `OutboundEnvelope` を error path で返す retry 可能な `send` contract を反映する。
- `remote-core-association-state-machine` / `remote-core-extension`: watermark の意味を決定して文書化する。実装が internal drain を止めないために `BackpressureSignal::Notify` を残すなら正式化する。そうでなければ `Notify` を削除し、`Apply` が drain path を deadlock させないことを証明する。
- `remote-adaptor-std-io-worker`: `RemoteShared` run-loop architecture と衝突する古い `AssociationRegistry` / `StdRemoting` requirement を削除する。

## Capabilities

### 追加する Capability

- **`cluster-adaptor-std-remote-delivery`**
  - cluster std integration は、cluster 向け remote reference が actor-core provider resolution と std remote adapter 配送経路を使うことを証明する。
  - remoting lifecycle subscription は provider lifetime 中に保持されるか、caller が保持できる guard として返る。

- **`remote-adaptor-std-extension-installer`**
  - `RemotingExtensionInstaller` は `ActorSystemConfig::with_extension_installers` 経由で install でき、caller は install 後も同じ handle から `remote()` / `shutdown_and_join()` を呼べる。
  - remote lifecycle showcase は direct install ではなく config install 経路を示す。

### 変更する Capability

- **`actor-core-extension-installers`**
  - stateful installer を caller-retained shared handle として登録できるようにし、application code が `ExtensionInstaller::install` を直接呼ばなくても bootstrap-time install と post-install control を両立できる。

- **`remote-adaptor-std-tcp-transport`**
  - `TcpRemoteTransport::send` は常時失敗ではなく、outbound envelope frame を serialize して enqueue する。
  - TCP runtime は connection-loss event を remote event channel へ emit する。

- **`remote-adaptor-std-io-worker`**
  - inbound delivery は `RemoteShared` buffered envelopes から local actor system へ bridge される。
  - pivot 前の adapter-driver requirements は `RemoteShared` run-loop wiring へ置き換える。

- **`remote-adaptor-std-provider-dispatch`**
  - actor-core provider dispatch 経由で解決された remote actor ref は、std remote event sender と real transport send path に到達しなければならない。

- **`remote-core-extension`**
  - live spec を `RemoteShared` `&self` remoting と選択した watermark signal の意味に揃える。
  - 必要なら raw lock を露出せず、event semantics を重複させない小さい shared event-step API を追加できる。

- **`remote-core-association-state-machine`**
  - watermark backpressure の意味を内部整合させる。

- **`remote-core-transport-port`**
  - live spec は現在の retry 可能な `send` return contract を反映する。

## 影響

**影響を受けるコード**

- `modules/remote-adaptor-std/src/std/transport/tcp/*`
- `modules/remote-adaptor-std/src/std/extension_installer/remoting_extension_installer.rs`
- `modules/remote-adaptor-std/src/std/tokio_remote_event_receiver.rs`
- `modules/remote-adaptor-std/src/std/provider/*`
- `modules/actor-core/src/core/kernel/actor/extension/*`
- `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs`
- `modules/remote-core/src/core/extension/*`
- `modules/remote-core/src/core/transport/*`
- `modules/remote-core/src/core/association/*`
- `modules/cluster-adaptor-std/src/std/local_cluster_provider_ext.rs`
- `showcases/std/legacy/remote_lifecycle/main.rs`
- `showcases/std/legacy/tests/remote_lifecycle_surface.rs`
- remote / cluster integration tests
- `openspec/specs/` 配下の live OpenSpec specs

**公開 API 影響**

- `ExtensionInstallers` は caller-retained shared installer を登録できる API を追加する可能性がある。
- `RemoteTransport::send` の意味を明確化する。実装は既に retry 可能な error shape を使っている。
- `BackpressureSignal::Notify` は正式化または削除する。public enum なので API decision として扱う。
- `subscribe_remoting_events` は subscription guard を返すか、provider state に保存する必要がある。guard を返す場合、caller はそれを保持しなければならない。

**挙動影響**

- remote actor ref がサポート対象 payload を std TCP nodes 間で配送できる。
- inbound remote message は `Remote` 内 buffer に留まらず、local actor mailbox に届く。
- cluster remoting lifecycle update が subscription 後に実際に観測される。
- remote lifecycle showcase は actor system bootstrap 時に remote extension を install し、`main` から direct install しない。

## 対象外

- Pekko Artery byte-compatible protocol。
- 明示的 codec contract なしの任意 `AnyMessage` serialization。
- 既存 `ConnectionLost` event emission を超える failure detector heartbeat redesign。
- per-authority channel optimization、zero-copy payload transport、redelivery ACK window completion。
- adapter runtime internals の隠蔽。これは `hide-remote-adaptor-runtime-internals` の scope に残す。
