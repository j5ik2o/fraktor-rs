## 文脈

pivot 後の architecture の ownership direction は正しい。

```text
adapter I/O task
  -> RemoteEvent
  -> RemoteShared::run
  -> Remote::handle_remote_event
  -> Association state/effects
  -> RemoteTransport port
```

足りないのは、この loop の外側の edge である。outbound は `RemoteTransport::send` まで到達して止まる。inbound は `Remote::inbound_envelopes` まで到達して止まる。cluster lifecycle subscription は RAII subscription handle を drop するため、登録直後に解除される。

もう 1 つ、remote lifecycle showcase は extension install の正規経路を外している。`ActorSystemConfig::with_extension_installers` は既に存在するが、現在の `ExtensionInstallers::with_extension_installer` は installer を値で受け取り registry 内へ隠す。`RemotingExtensionInstaller` は install 後も `remote()` / `spawn_run_task()` / `shutdown_and_join()` を呼ぶ stateful installer なので、caller が同じ handle を保持できないと direct install へ逃げやすい。

## 設計判断

### 判断 1: `RemoteTransport::send` は同期のまま維持する

`RemoteTransport::send` を async にしない。core run loop は `Remote::handle_remote_event` の bounded step の中で、`Remote` への mutable access を持った状態から `send` を呼ぶ。ここで await すると runtime concern が `remote-core` に入り、pivot 後の CQS boundary が崩れる。

そのため `TcpRemoteTransport::send` は bounded work のみを行う。

1. transport が running であることを確認する。
2. `OutboundEnvelope` を `EnvelopePdu` に変換する。
3. 既存 peer writer を探す。
4. `WireFrame::Envelope` を writer に enqueue する。
5. 失敗時は元 envelope を返す。

Connection establishment は adapter / runtime 側の仕事として残す。writer が準備できていない場合は `ConnectionClosed` が正しい結果であり、`Remote` は re-enqueue できる。後続 task が peer へ接続し、別 event を送って core を wake する。

### 判断 2: payload serialization は明示的にする

`AnyMessage` は任意の `dyn Any` を保持する。actor-core には汎用 serializer がない。最初の delivery proof で、任意 payload が serialize 可能であるかのように扱ってはならない。

この change では小さな std adapter codec contract を追加または選択する。

- サポート対象: `bytes::Bytes`、`Vec<u8>`、および実装で明示登録された wire-safe payload type。
- 未サポート: 観測可能な send / codec error を返し、error path で元 envelope を保持する。
- inbound supported payload は合意した型で `AnyMessage` を復元する。最初の proof type は `Bytes` を推奨する。

serializer registry を実装する場合、それは adapter-owned で public behavior 経由で test する。`remote-core` に serde 依存や application message type knowledge を持ち込まない。

### 判断 3: 現在の run future が不透明なら event-step hook を追加する

inbound delivery の最も自然な hook point は core event step の直後である。現在 `RemotingExtensionInstaller::spawn_run_task` は次を呼ぶ。

```rust
remote.run(&mut receiver).await
```

この形だと adapter は event ごとの完了を観測できない。現在の API で event 後に inbound envelopes を drain できない場合は、次のような小さい `RemoteShared` API を追加する。

```rust
pub fn handle_event(&self, event: RemoteEvent) -> Result<bool, RemotingError>
```

bool は「event loop を停止すべきか」を示す adapter orchestration signal に限る。この API は内部で `Remote::handle_remote_event` と `Remote::should_stop_event_loop` に委譲し、event matching や association logic を `RemoteShared` に移さない。raw `SharedLock<Remote>` も露出しない。

adapter は次の loop を所有できる。

```text
while let Some(event) = receiver.recv().await:
  let stop = remote_shared.handle_event(event)?;
  deliver(remote_shared.drain_inbound_envelopes())?;
  if stop { break }
```

既存 futures により適した最小 API があるならそれを使ってよい。ただし責務分担は守る。core は remoting state を処理し、adapter は actor-system delivery を処理する。

### 判断 4: delivery bridge は std adapter に置く

`InboundEnvelope` から local actor mailbox への bridge には actor-system / provider access と std runtime error handling が必要である。これは `remote-adaptor-std` の責務である。

bridge は次を満たす。

- recipient path を local actor system / provider で resolve する。
- reconstructed `AnyMessage` を `try_tell` または同等の同期 mailbox path へ送る。
- failure は actor-core convention に従って dead letters または visible adapter error path へ流す。
- remote lock を保持したまま actor へ delivery しない。

### 判断 5: 実装が `Apply` の安全性を証明しない限り `BackpressureSignal::Notify` を正式化する

archived proposal は high watermark で `BackpressureSignal::Apply` を呼ぶことを要求していた。現在の queue model では、`Apply` は user lane を pause する。同じ drain helper が user message を dequeue する前に `Apply` すると、自分自身が queue を drain できなくなり、low watermark release に到達できない可能性がある。

そのため推奨方針は次の通り。

- `Apply` / `Release` は、transport または upper layer が user traffic を意図的に pause / resume する用途に残す。
- 内部 high-watermark 観測と instrumentation には `Notify` を使う。
- `Release` は過去の real `Apply` を release する場合だけ使う。`Notify` の release ではない。
- live spec にこの差を明記する。

実装が `Notify` を削除する場合、high watermark が internal drain を deadlock させないことを test で証明する。

### 判断 6: remoting event subscription を保持する

`EventStreamSubscription` は `Drop` 時に unsubscribe する。helper 内の `_subscription` ローカル変数に入れて `()` を返すと、永続 subscription にはならない。

この change では次のどちらかを選ぶ。

- `subscribe_remoting_events` から `EventStreamSubscription` を返し、caller が lifetime を所有する。
- `LocalClusterProvider` / shared provider state に subscription を保存する。

guard を返す方が小さい API である。state に保存する方が「一度 subscribe したら忘れてよい」ergonomics になる。実装は既存 cluster provider の ownership pattern に合わせる。

### 判断 7: stateful extension installer は caller-retained shared handle で登録できるようにする

remote extension は application `main` から `installer.install(&system)` を直接呼ばない。`ActorSystemConfig::with_extension_installers` に登録し、`ActorSystem::create_with_config` 中の bootstrap-time install に統一する。

一方で `RemotingExtensionInstaller` は install 後に caller が lifecycle control を続ける必要がある。したがって actor-core 側は、caller が shared installer handle を保持したまま `ExtensionInstallers` に登録できる API を提供する。例としては次のいずれかを許容する。

- `ExtensionInstallers::with_shared_extension_installer(handle.clone())` のように shared handle を直接登録する。
- local trait 実装により `ArcShared<RemotingExtensionInstaller>` または `ArcShared<dyn ExtensionInstaller>` を `ExtensionInstaller` として登録できるようにする。
- remote-adaptor-std 側に、shared handle と `ExtensionInstallers` 登録を同時に作る小さい helper を置く。

どの形でも、caller に見える usage は次の性質を満たす。

```text
let installer = shared RemotingExtensionInstaller
let installers = ExtensionInstallers::default().with_...(installer.clone())
let config = ActorSystemConfig::new(...).with_extension_installers(installers)
let system = ActorSystem::create_with_config(&props, config)
let remote = installer.remote()
remote.start()
installer.shutdown_and_join()
```

registry が shared handle をさらに別 allocation で包んでも構わないが、install される対象と caller が保持する対象は同じ `RemotingExtensionInstaller` state でなければならない。`remote()` が `NotStarted` のままになる clone / wrapper split は許容しない。

### 判断 8: remote actor-ref provider も ActorSystemConfig 経由で登録する

remote delivery の E2E は `StdRemoteActorRefProvider` を直接 new して呼ぶ test では完了扱いにしない。user-facing 経路は `ActorSystem::resolve_actor_ref(remote path)` から始まるため、std remote adapter は actor-core の provider installer 経路に接続されなければならない。

actor-core には `ActorSystemConfig::with_actor_ref_provider_installer` がある。remote-adaptor-std はこれを使って `StdRemoteActorRefProvider` を actor system に登録する installer または builder helper を提供する。installer は local provider を wrap し、remote authority の path は `RemoteActorRefSender` へ、local authority の path は既存 local provider へ振り分ける。

受け入れ条件は次の通り。

- `ActorSystem::create_with_config` に渡した config だけで remote-aware provider が install される。
- `ActorSystem::resolve_actor_ref(remote path)` が `StdRemoteActorRefProvider` を通る。
- resolved `ActorRef` への tell が `RemoteEvent::OutboundEnqueued` を adapter event sender へ push する。
- test は `StdRemoteActorRefProvider::actor_ref` を直接呼ぶだけで済ませない。

## 処理の流れ

outbound のサポート対象 payload:

```text
remote ActorRef.tell(Bytes)
  -> RemoteActorRefSender::send
  -> try_send(RemoteEvent::OutboundEnqueued)
  -> RemoteShared event step
  -> Association::enqueue / next_outbound
  -> TcpRemoteTransport::send
  -> EnvelopePdu
  -> TcpClient writer_tx
  -> peer inbound worker
```

inbound のサポート対象 payload:

```text
peer TcpClient/TcpServer reads WireFrame::Envelope
  -> run_inbound_dispatch
  -> RemoteEvent::InboundFrameReceived
  -> RemoteShared event step
  -> Remote buffers InboundEnvelope
  -> std delivery bridge drains
  -> ActorSystem resolves recipient
  -> local ActorRef.try_tell
```

connection loss:

```text
tcp read/write task exits unexpectedly
  -> RemoteEvent::ConnectionLost { authority, cause, now_ms }
  -> Remote::handle_remote_event
  -> Association::gate / recover
  -> lifecycle effects / handshake retry
```

remote lifecycle showcase:

```text
RemotingExtensionInstaller shared handle
  -> ExtensionInstallers
  -> ActorSystemConfig::with_extension_installers
  -> ActorSystem::create_with_config
  -> ExtensionInstallers::install_all
  -> installer.remote() from retained handle
  -> remote.start()
  -> installer.shutdown_and_join()
```

remote actor-ref provider wiring:

```text
remote provider installer
  -> ActorSystemConfig::with_actor_ref_provider_installer
  -> ActorSystem::create_with_config
  -> ActorSystem::resolve_actor_ref(remote path)
  -> StdRemoteActorRefProvider
  -> RemoteActorRefSender
  -> RemoteEvent::OutboundEnqueued
```

## リスク

- connection establishment を完全に manual のままにすると、cluster e2e test が test-only preconnect plumbing でしか通らない可能性がある。public または adapter-owned lifecycle の中で setup を明示する。
- 未サポート payload を silent drop すると delivery test が false confidence を与える。未サポート serialization は観測可能にする。
- remote write lock を保持したまま inbound delivery すると、actor callback と lock-order hazard を作りうる。drain してから lock 外で deliver する。
- spec drift を同じ change で直さないと、後続作業が pivot 前の pattern を再導入する。
- stateful installer の shared handle 登録を用意しないと、showcase や user code が `install(&system)` direct call に戻り、bootstrap-time install order と root-start ordering を迂回してしまう。
- remote actor-ref provider の config wiring を要求しないと、E2E が provider の direct unit test に留まり、`ActorSystem::resolve_actor_ref` から remote path を解決する user-facing 経路が未証明になる。

## 検証方針

- `OutboundEnvelope -> EnvelopePdu` 変換の unit test。recipient path、sender path、correlation id、priority、未サポート payload error を含める。
- `TcpRemoteTransport::send` が既存 connection の peer へ `WireFrame::Envelope` を出す TCP integration test。
- サポート対象 payload delivery が local actor mailbox に届く two-node remote-adaptor test。
- TCP task failure が `RemoteEvent::ConnectionLost` を emit し、core recovery path が起動する test。
- cluster-adaptor test。helper return 後も remoting lifecycle subscription が active であることを証明する。
- remote lifecycle showcase surface test。`RemotingExtensionInstaller` が `ActorSystemConfig::with_extension_installers` 経由で install され、`main` に direct `installer.install(&system)` が残らないことを確認する。
- actor-core provider wiring test。remote-aware provider が `ActorSystemConfig::with_actor_ref_provider_installer` 経由で install され、`ActorSystem::resolve_actor_ref(remote path)` から `RemoteEvent::OutboundEnqueued` まで到達することを確認する。
- `ClusterApi::get` / `GrainRef` または既存 provider path 経由の cluster 向け remote delivery test。
- spec validation、package tests、repo-wide CI。
