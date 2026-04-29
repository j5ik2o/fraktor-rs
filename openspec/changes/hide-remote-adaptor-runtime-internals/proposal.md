## Why

`remote-adaptor-std` は `StdRemoting` の撤去により core lifecycle の代替入口を失ったが、まだ runtime driver や TCP frame 部品が public surface に露出している。利用者が `Remote` / `RemoteTransport` / actor system extension だけを理解すればよい設計にするため、std adapter 内部の配線部品をモジュール内へ隠蔽する。

特に `StdRemoteActorRefProvider` は core wrapper ではなく actor-core と remote-core をつなぐ adapter bridge として妥当だが、現状の constructor は `local_provider`、`remote_provider`、`transport`、`resolve_cache`、`event_publisher` を利用者に渡させるため認知負荷が高い。最終的には installer / config 側で組み立て、通常利用者の API から低レベル配線を消す。

## What Changes

### 1. `remote-adaptor-std` の public surface を分類する

公開 API を次の 3 区分に分ける。

- 利用者向け adapter 境界: `TcpRemoteTransport`、`RemotingExtensionInstaller`、設定から生成される provider / installer 類
- adapter bridge: `StdRemoteActorRefProvider` のように actor-core の extension point へ差し込む型
- runtime internal: association runtime、watcher actor、TCP client/server/frame、sender 実装、再接続・再送制御など

runtime internal は原則 `pub(crate)` または private module に落とし、crate 外から直接組み立てさせない。

### 2. association / watcher / TCP frame の内部部品を隠す

以下は `remote-adaptor-std` 内部の runtime driver であり、利用者に公開しない。

- `AssociationRegistry`
- `AssociationShared`
- `HandshakeDriver`
- `RestartCounter`
- `ReconnectBackoffPolicy`
- `SystemMessageDeliveryState`
- `InboundQuarantineCheck`
- `run_inbound_dispatch`
- `run_inbound_task_with_restart_budget`
- `run_outbound_loop`
- `run_outbound_loop_with_reconnect`
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

必要な integration test は crate 内部テストへ移すか、public API 経由の外部テストに置き換える。

### 3. `StdRemoteActorRefProvider` の低レベル constructor を隠す

`StdRemoteActorRefProvider` は actor-core の `ActorRefProvider` に remote 解決を差し込む adapter bridge として残す。ただし利用者が低レベル依存を手で渡す constructor は公開 API から外す。

代わりに、actor system extension installer または std remote configuration から provider を構築する経路を用意する。外部利用者は「remote を有効化する設定」または「remote extension installer」を指定するだけでよく、`LocalActorRefProvider`、`RemoteActorRefProvider`、`TcpRemoteTransport`、`ActorRefResolveCache`、`EventPublisher` の組み合わせを意識しない。

### 4. showcase と public surface test を更新する

showcase は低レベル runtime 部品を直接 import しない。remote lifecycle showcase は `Remote` と `RemotingExtensionInstaller`、または設定経由の remote 有効化だけを示す。

remote routee expansion showcase は、`StdRemoteActorRefProvider::new(...)` を直接呼ぶ形をやめ、installer / actor system 経由で provider が組み立てられる形へ寄せる。

## Capabilities

### New Capabilities

- **`remote-adaptor-std-public-surface`**
  - `remote-adaptor-std` の public API は利用者向け adapter 境界に限定される
  - runtime driver 部品は crate 外から直接 import できない
  - provider bridge の低レベル配線は installer / config 側へ隠蔽される

### Modified Capabilities

- **`remote-core-package`**
  - `remote-core` は引き続き core API と Port trait を提供する
  - `remote-adaptor-std` は core API の代替入口ではなく、Port 実装と actor system 配線だけを提供する

## Impact

**影響を受けるコード:**

- `modules/remote-adaptor-std/src/std.rs`
- `modules/remote-adaptor-std/src/std/tcp_transport.rs`
- `modules/remote-adaptor-std/src/std/provider.rs`
- `modules/remote-adaptor-std/src/std/association_runtime.rs`
- `modules/remote-adaptor-std/src/std/watcher_actor.rs`
- `modules/remote-adaptor-std/src/std/extension_installer.rs`
- `modules/remote-adaptor-std/src/std/provider/dispatch.rs`
- `modules/remote-adaptor-std/src/std/provider/remote_routee_expansion.rs`
- `showcases/std/remote_lifecycle/`
- `showcases/std/remote_routee_expansion/`
- public surface tests

**公開 API 影響:**

- runtime internal 型の re-export を削除する破壊的変更。
- `StdRemoteActorRefProvider` の低レベル constructor を public API から外すか、より高レベルな builder / installer 経由へ置換する破壊的変更。
- `TcpRemoteTransport` と `RemotingExtensionInstaller` は利用者向け adapter 境界として維持する。

**挙動影響:**

- remote lifecycle / path resolution / routing の挙動は維持する。
- 変更対象は主に公開範囲と組み立て経路であり、wire protocol や association state machine の意味論は変えない。

## Non-goals

- `remote-core` の state machine や wire protocol の再設計
- `TcpRemoteTransport` の削除
- `StdRemoteActorRefProvider` の概念自体の削除
- remote payload serialization の完成
- cluster adaptor や stream adaptor の公開 surface 整理
- 後方互換 shim、deprecated alias、旧 constructor の残置
