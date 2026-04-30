## Context

`remote-core` は no_std の state machine / Port trait / wire model を持ち、`remote-adaptor-std` は TCP、Tokio task、actor-core extension point への接続を担当する。直前の整理で `StdRemoting` は削除され、利用者は `remote-core::Remote` に `TcpRemoteTransport` を差し込む構造へ寄った。

残る問題は、`remote-adaptor-std` の runtime 実装部品が crate 外へ public re-export されていることにある。これは `StdRemoting` のような core lifecycle wrapper ではないが、利用者に「どの部品を直接組み立てるべきか」を考えさせ、Remote の学習コストを上げる。

現在の構造は概ね次の形である。

```text
remote-adaptor-std::std
├─ extension_installer
│  └─ RemotingExtensionInstaller          利用者向け境界
├─ transport
│  └─ tcp
│     ├─ TcpRemoteTransport              利用者向け境界
│  ├─ TcpClient / TcpServer               runtime internal
│  └─ WireFrame / WireFrameCodec / ...    runtime internal
├─ provider
│  ├─ StdRemoteActorRefProvider           adapter bridge
│  ├─ RemoteRouteeExpansion               手動配線 API
│  └─ RemoteActorRefSender                runtime internal
├─ association                     runtime internal
└─ watcher_actor                          runtime internal
```

目指す構造は次の形である。

```text
利用者が見る API
┌────────────────────────────────────────────┐
│ remote-core::Remote                         │
│ remote-adaptor-std::TcpRemoteTransport      │
│ remote-adaptor-std::RemotingExtension...    │
│ remote を有効化する installer/config        │
└────────────────────────────────────────────┘

crate 内で閉じる API
┌────────────────────────────────────────────┐
│ association                          │
│ watcher_actor                               │
│ tcp client/server/frame codec               │
│ remote actor ref sender                     │
│ provider の低レベル配線                     │
└────────────────────────────────────────────┘
```

## Goals / Non-Goals

**Goals:**

- `remote-adaptor-std` の public API を利用者向け adapter 境界に絞る。
- runtime driver / TCP frame / watcher actor / sender 実装を crate 内へ隠す。
- `StdRemoteActorRefProvider` の存在は維持しつつ、低レベル constructor を通常利用者の public surface から外す。
- remote lifecycle、remote actor ref resolution、remote routee expansion の挙動は維持する。
- 後方互換 shim を残さない。

**Non-Goals:**

- `remote-core` の state machine、wire protocol、transport trait の再設計。
- `StdRemoteActorRefProvider` の概念削除。
- `TcpRemoteTransport` の削除。
- payload serialization の完成。
- cluster / stream / persistence adaptor の公開 surface 整理。

## Decisions

### Decision 1: `TcpRemoteTransport` だけを transport module の利用者向け型として残す

`TcpRemoteTransport` は `RemoteTransport` の std 実装なので public に残す。`TcpClient`、`TcpServer`、`WireFrame`、`WireFrameCodec`、`FrameCodecError`、`InboundFrameEvent` は transport 実装の内部構成要素であり public re-export しない。

この変更に伴い、`TcpRemoteTransport` の public inherent method が内部型を返している場合は `pub(crate)` 化する。代表例は以下である。

- `take_inbound_receiver() -> Option<UnboundedReceiver<InboundFrameEvent>>`
- `clients() -> &BTreeMap<String, TcpClient>`
- runtime driver 専用の connection / handshake 補助メソッド

利用者は `Remote::start` / `Remote::shutdown` / `RemoteTransport` 経由で transport を扱う。

### Decision 2: association runtime と watcher actor は crate 内部 API にする

`AssociationRegistry`、`AssociationShared`、`HandshakeDriver`、`RestartCounter`、`ReconnectBackoffPolicy`、`SystemMessageDeliveryState`、`InboundQuarantineCheck`、`run_inbound_dispatch`、`run_outbound_loop` は runtime driver の部品であり、crate 外の利用者が直接依存すべきではない。

`WatcherActor`、`WatcherActorHandle`、`run_heartbeat_loop` も同様に、remote runtime の内部タスクとして扱う。watcher を利用者が直接起動する形は、Remote の利用モデルを分断するため避ける。

### Decision 3: `StdRemoteActorRefProvider` は bridge として残し、組み立ては installer / config に寄せる

`StdRemoteActorRefProvider` は `StdRemoting` と違い、core API の代替入口ではない。actor-core の `ActorRefProvider` extension point と remote-core の `RemoteActorRefProvider` を接続し、local loopback / remote dispatch を判断する adapter bridge である。

ただし、現在の `new(...)` は以下を利用者に渡させる。

- `UniqueAddress`
- `ActorRefProviderHandleShared<LocalActorRefProvider>`
- `Box<dyn RemoteActorRefProvider + Send + Sync>`
- `SharedLock<TcpRemoteTransport>`
- `ActorRefResolveCache<RemoteActorRef>`
- `EventPublisher`

これは低レベルすぎる。実装では次のいずれかを採用する。

- `StdRemoteActorRefProvider::new` を `pub(crate)` にして、extension installer が組み立てる。
- public constructor を残す場合でも、利用者向けには `RemoteProviderInstaller` / `RemoteActorRefProviderInstaller` のような高レベル installer を追加し、低レベル constructor は docs / showcase から消す。

初期方針は `new` の `pub(crate)` 化を優先する。公開 API として本当に provider bridge が必要であれば、低レベル依存を直接受け取らない builder を後から追加する。

### Decision 4: `RemoteRouteeExpansion` は手動配線 API から public helper へ降格する

`RemoteRouteeExpansion` は remote router の構築 helper だが、現在は `StdRemoteActorRefProvider` を手動で作って渡す前提になっている。これは remote routee を使う利用者に provider wiring を見せてしまう。

実装では以下の順で整理する。

1. `RemoteRouteeExpansion` が `StdRemoteActorRefProvider::new` を必要としない public 経路に変えられるか確認する。
2. できない場合は `RemoteRouteeExpansion` を `pub(crate)` に落とし、routee expansion は actor system / provider installer の内部責務にする。
3. showcase は public API 経由に更新し、低レベル provider 構築を例示しない。

### Decision 5: tests は公開契約テストと内部テストに分ける

crate 外 integration test は public API のみを使う。内部 runtime driver の細かいテストは `src/**/tests.rs` に残し、private module / `pub(crate)` item を直接検証する。

公開契約テストでは以下を確認する。

- hidden internal 型を external crate から import できない。
- `TcpRemoteTransport` と `RemotingExtensionInstaller` は external crate から利用できる。
- routee / provider の showcase は低レベル constructor を参照しない。

## Alternatives Considered

### Alternative A: `StdRemoteActorRefProvider` を削除する

却下。これは core wrapper ではなく、actor-core の provider extension point へ remote を接続する adapter bridge である。削除すると local / remote dispatch の責務が別の場所へ漏れるだけで、認知負荷は下がらない。

### Alternative B: すべての runtime 部品を public のまま docs で internal と説明する

却下。Rust の public API は互換性と利用可能性を意味するため、docs だけで internal と説明しても利用者の認知負荷は下がらない。正式リリース前なので、公開範囲を破壊的に縮小する。

### Alternative C: `TcpRemoteTransport` も完全に隠す

却下。`Remote` に差し込む具体 transport は adapter 実装としてユーザが選ぶ境界であり、`TcpRemoteTransport` は public に残す価値がある。ただし内部 TCP client/server/frame は隠す。

## Risks / Trade-offs

- public re-export 削除により existing showcase / external tests が壊れる。これは正式リリース前の破壊的変更として許容する。
- `StdRemoteActorRefProvider::new` を隠すと、現在の remote routee expansion showcase は作り替えが必要になる。
- 内部型を隠す過程で public method が private type を返せなくなるため、`TcpRemoteTransport` の inherent method 可視性も同時に見直す必要がある。
- provider installer の設計を厚くしすぎると、新しい wrapper 問題を作る。installer は「配線を隠す」だけに留め、core lifecycle の代替入口にはしない。

## Migration Plan

1. public re-export 一覧を固定し、残す型 / 隠す型をテストで明文化する。
2. `transport::tcp` の public re-export を `TcpRemoteTransport` のみに縮小する。
3. `association` と `watcher_actor` を `pub(crate)` module または internal-only re-export に変更する。
4. `RemoteActorRefSender` を provider module 内部へ隠す。
5. `StdRemoteActorRefProvider::new` を `pub(crate)` 化し、installer / config 経由の構築経路を追加または既存 installer に統合する。
6. `RemoteRouteeExpansion` の public 必要性を再評価し、手動 provider 配線が必要なら public API から外す。
7. showcase と public surface tests を public API 経由へ更新する。
8. `rtk cargo test -p fraktor-remote-adaptor-std-rs` と `rtk ./scripts/ci-check.sh ai all` で確認する。

## Open Questions

- `StdRemoteActorRefProvider` は最終的に public 型として残すべきか、actor system extension に完全に隠すべきか。
- routee expansion は `RemoteRouterConfig` の public API としてどの層に置くべきか。
- `TcpRemoteTransport::connect_peer` / `send_handshake` は public debugging API として残す価値があるか、association runtime 専用に閉じるべきか。
