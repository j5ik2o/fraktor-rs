## Why

`remote-adaptor-std` は `StdRemoting` の撤去により core lifecycle の代替入口を失ったが、まだ runtime driver や TCP frame 部品が public surface に露出している。加えて user-facing showcase は `RemotingExtensionInstaller` を `ActorSystemConfig::with_extension_installers` に渡した後で、caller に `installer.remote()` / `remote.start()` / `installer.shutdown_and_join()` を直接呼ばせている。これは Pekko のように ActorSystem configuration で remoting を有効化する利用モデルと合わず、remote lifecycle control が application `main` に漏れている。

利用者が理解すべき入口は、`TcpRemoteTransport` と `RemotingExtensionInstaller` を actor system config に渡すことだけに絞る。`RemoteShared` の start / shutdown / addresses などの意味論は `remote-core` に残し、いつそれを呼ぶか、tokio run task をどう起動・停止するか、inbound delivery をどう ActorSystem に戻すかは `remote-adaptor-std` の installer / ActorSystem lifecycle 側へ閉じる。

特に `StdRemoteActorRefProvider` は core wrapper ではなく actor-core と remote-core をつなぐ adapter bridge として妥当だが、現状の constructor は `local_provider`、`remote_provider`、`event_sender`、`resolve_cache`、`event_publisher`、monotonic epoch を利用者に渡させるため認知負荷が高い。最終的には installer / config 側で組み立て、通常利用者の API から低レベル配線を消す。

## What Changes

### 1. `remote-adaptor-std` の public surface を分類する

公開 API を次の 3 区分に分ける。

- 利用者向け adapter 境界: `TcpRemoteTransport`、`RemotingExtensionInstaller`、設定から生成される provider installer / config 類
- adapter bridge: `StdRemoteActorRefProvider` のように actor-core の extension point へ差し込む型
- runtime internal: association runtime、watcher actor、TCP client/server/frame、sender 実装、再接続・再送制御など

runtime internal は原則 `pub(crate)` または private module に落とし、crate 外から直接組み立てさせない。

### 2. association / watcher / TCP frame の内部部品を隠す

以下は `remote-adaptor-std` 内部の runtime driver または低レベル配線であり、利用者に公開しない。

- `run_inbound_dispatch`
- `TokioMpscRemoteEventReceiver`
- `WatcherActor`
- `WatcherActorHandle`
- `SubmitError`
- `run_heartbeat_loop`
- `TcpClient`
- `TcpServer`
- `WireFrame`
- `WireFrameCodec`
- `FrameCodecError`
- `InboundFrameEvent`
- `RemoteActorRefSender`
- `PathRemoteActorRefProvider` などの低レベル remote provider plumbing

必要な integration test は crate 内部テストへ移すか、public API 経由の外部テストに置き換える。

### 3. `StdRemoteActorRefProvider` の低レベル constructor を隠す

`StdRemoteActorRefProvider` は actor-core の `ActorRefProvider` に remote 解決を差し込む adapter bridge として残す。ただし利用者が低レベル依存を手で渡す constructor は公開 API から外す。

代わりに、actor system extension installer または std remote configuration から provider を構築する経路を用意する。外部利用者は `RemotingExtensionInstaller` / provider installer / config を `ActorSystemConfig` に渡すだけでよく、`LocalActorRefProvider`、`RemoteActorRefProvider`、`RemoteEvent` sender、`ActorRefResolveCache`、`EventPublisher` の組み合わせを意識しない。

### 4. remote lifecycle control を ActorSystem / installer 内部へ移す

`RemotingExtensionInstaller::new(transport, remote_config)` を shared installer として `ActorSystemConfig::with_extension_installers` に渡した時点で、remote を有効化する intent は十分に表現されている。通常の application code や showcase は、install 後に `installer.remote()` で `RemoteShared` を取り出して `remote.start()` を呼んだり、`spawn_run_task()` / `shutdown_and_join()` を lifecycle 手順として呼んだりしてはならない。

`RemotingExtensionInstaller` または ActorSystem lifecycle hook は、install 後に core の `RemoteShared::start()` を内部で呼び、std adapter の run task を起動する。ActorSystem termination 時には core の shutdown semantics を呼び出し、event loop wake と tokio `JoinHandle` の完了観測まで adapter 側で行う。

`remote()` は診断・内部テスト用に残す余地はあるが、利用者向け startup API として showcase / docs / public surface test に出さない。

### 5. showcase と public surface test を更新する

showcase は低レベル runtime 部品を直接 import しない。remote lifecycle showcase は `TcpRemoteTransport` と `RemotingExtensionInstaller` を `ActorSystemConfig` に渡す形だけを示し、`installer.remote()` / `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を含めない。

remote routee expansion showcase は、`StdRemoteActorRefProvider::new(...)` を直接呼ぶ形をやめ、installer / actor system 経由で provider が組み立てられる形へ寄せる。

## Capabilities

### New Capabilities

- **`remote-adaptor-std-public-surface`**
  - `remote-adaptor-std` の public API は利用者向け adapter 境界に限定される
  - runtime driver 部品は crate 外から直接 import できない
  - provider bridge の低レベル配線は installer / config 側へ隠蔽される
  - remote lifecycle control は application `main` から消え、installer / ActorSystem lifecycle 側へ隠蔽される

### Modified Capabilities

- **`remote-core-package`**
  - `remote-core` は引き続き core API と Port trait を提供する
  - `remote-adaptor-std` は core lifecycle semantics の代替実装を持たず、ActorSystem に接続された adapter として core API を内部で呼び出す
- **`remote-adaptor-std-extension-installer`**
  - config install 後の remote start / run task / shutdown join を installer / ActorSystem lifecycle が所有する
  - caller-retained installer handle は config 登録や診断のために使えても、通常の startup sequence には不要にする

## Impact

**影響を受けるコード:**

- `modules/remote-adaptor-std/src/std.rs`
- `modules/remote-adaptor-std/src/std/transport.rs`
- `modules/remote-adaptor-std/src/std/provider.rs`
- `modules/remote-adaptor-std/src/std/association.rs`
- `modules/remote-adaptor-std/src/std/watcher_actor.rs`
- `modules/remote-adaptor-std/src/std/extension_installer.rs`
- `modules/remote-adaptor-std/src/std/provider/dispatch.rs`
- `modules/remote-adaptor-std/src/std/provider/remote_routee_expansion.rs`
- `showcases/std/legacy/remote_lifecycle/`
- routee / routing showcase または public surface tests
- public surface tests

**公開 API 影響:**

- runtime internal 型の re-export を削除する破壊的変更。
- `StdRemoteActorRefProvider` の低レベル constructor を public API から外すか、より高レベルな builder / installer 経由へ置換する破壊的変更。
- `TcpRemoteTransport` と `RemotingExtensionInstaller` は利用者向け adapter 境界として維持する。
- user-facing startup API から `installer.remote()` / `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を外す破壊的変更。

**挙動影響:**

- remote lifecycle / path resolution / routing の意味論は維持する。
- `remote-core` の state machine / lifecycle semantics は変えず、呼び出し主体を application `main` から installer / ActorSystem lifecycle へ移す。
- wire protocol や association state machine の意味論は変えない。

## Non-goals

- `remote-core` の state machine や wire protocol の再設計
- `remote-core` の lifecycle semantics を std adapter へ複製すること
- `TcpRemoteTransport` の削除
- `StdRemoteActorRefProvider` の概念自体の削除
- remote payload serialization の完成
- cluster adaptor や stream adaptor の公開 surface 整理
- 後方互換 shim、deprecated alias、旧 constructor の残置
