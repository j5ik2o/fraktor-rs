## Context

`remote-core` は no_std の state machine / Port trait / wire model を持ち、`remote-adaptor-std` は TCP、Tokio task、actor-core extension point への接続を担当する。直前の整理で `StdRemoting` は削除され、内部構造は `remote-core::RemoteShared` に `TcpRemoteTransport` を差し込む形へ寄った。

残る問題は、`remote-adaptor-std` の runtime 実装部品が crate 外へ public re-export されていることにある。これは `StdRemoting` のような core lifecycle wrapper ではないが、利用者に「どの部品を直接組み立てるべきか」を考えさせ、Remote の学習コストを上げる。

さらに現状の showcase / integration test は、`RemotingExtensionInstaller::new(transport, remote_config)` を `ActorSystemConfig::with_extension_installers` に渡した後で、caller が `installer.remote()` から `RemoteShared` を取り出し、`remote.start()` と `installer.shutdown_and_join()` を直接呼ぶ。この形は remote lifecycle operation を application `main` に露出しており、ActorSystem configuration で remoting を有効化する利用モデルと合わない。

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
│ remote-adaptor-std::TcpRemoteTransport      │
│ remote-adaptor-std::RemotingExtension...    │
│ remote を有効化する installer/config        │
│ ActorSystem lifecycle                       │
└────────────────────────────────────────────┘

crate 内で閉じる API
┌────────────────────────────────────────────┐
│ remote-core::Remote / RemoteShared          │
│ association                          │
│ watcher_actor                               │
│ tcp client/server/frame codec               │
│ remote actor ref sender                     │
│ run task / receiver / shutdown join         │
│ provider の低レベル配線                     │
└────────────────────────────────────────────┘
```

## Goals / Non-Goals

**Goals:**

- `remote-adaptor-std` の public API を利用者向け adapter 境界に絞る。
- runtime driver / TCP frame / watcher actor / sender 実装を crate 内へ隠す。
- `StdRemoteActorRefProvider` の存在は維持しつつ、低レベル constructor を通常利用者の public surface から外す。
- user-facing `main` から `installer.remote()` / `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を消し、installer / ActorSystem lifecycle が core lifecycle operation を内部で呼ぶ形にする。
- remote lifecycle、remote actor ref resolution、remote routee expansion の挙動は維持する。
- 後方互換 shim を残さない。

**Non-Goals:**

- `remote-core` の state machine、wire protocol、transport trait の再設計。
- `remote-core` の lifecycle semantics を std adapter に複製すること。
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

利用者は `RemoteShared` を取り出して `Remote::start` / `Remote::shutdown` を直接呼ばない。installer / ActorSystem lifecycle が core の `RemoteShared::start` / `RemoteShared::shutdown` を内部で呼び、利用者は actor system config と `ActorSystem::terminate` だけを扱う。

### Decision 2: association runtime と watcher actor は crate 内部 API にする

`run_inbound_dispatch`、`TokioMpscRemoteEventReceiver`、TCP frame codec、TCP client/server、remote actor ref sender は runtime driver の部品であり、crate 外の利用者が直接依存すべきではない。

`WatcherActor`、`WatcherActorHandle`、`run_heartbeat_loop` も同様に、remote runtime の内部タスクとして扱う。watcher を利用者が直接起動する形は、Remote の利用モデルを分断するため避ける。

### Decision 3: `StdRemoteActorRefProvider` は bridge として残し、組み立ては installer / config に寄せる

`StdRemoteActorRefProvider` は `StdRemoting` と違い、core API の代替入口ではない。actor-core の `ActorRefProvider` extension point と remote-core の `RemoteActorRefProvider` を接続し、local loopback / remote dispatch を判断する adapter bridge である。

ただし、現在の `new(...)` は以下を利用者に渡させる。

- `UniqueAddress`
- `ActorRefProviderHandleShared<LocalActorRefProvider>`
- `Box<dyn RemoteActorRefProvider + Send + Sync>`
- `Sender<RemoteEvent>`
- `ActorRefResolveCache<RemoteActorRef>`
- `EventPublisher`
- monotonic epoch

これは低レベルすぎる。実装では次のいずれかを採用する。

- `StdRemoteActorRefProvider::new` を `pub(crate)` にして、extension installer が組み立てる。
- public constructor を残す場合でも、利用者向けには `RemoteProviderInstaller` / `RemoteActorRefProviderInstaller` のような高レベル installer を追加し、低レベル constructor は docs / showcase から消す。

初期方針は `new` の `pub(crate)` 化を優先する。公開 API として本当に provider bridge が必要であれば、低レベル依存を直接受け取らない builder を後から追加する。

### Decision 4: remote lifecycle control は installer / ActorSystem lifecycle が所有する

`RemotingExtensionInstaller::new(transport, remote_config)` を `ActorSystemConfig::with_extension_installers` に渡した時点で、remote を有効化する intent は十分である。通常の application code は、install 後に `installer.remote()` で `RemoteShared` を取り出して `remote.start()` を呼んではならない。

実装では以下の分担にする。

- `remote-core`: `RemoteShared::start` / `shutdown` / `addresses` / event processing / transport port 呼び出しの意味論を持つ。
- `remote-adaptor-std`: `RemotingExtensionInstaller` または ActorSystem lifecycle hook が core operation を内部で呼ぶ。tokio `JoinHandle`、`TokioMpscRemoteEventReceiver`、inbound actor delivery、shutdown wake / join は adapter 側に残す。
- application `main`: `TcpRemoteTransport` と `RemoteConfig` から作った `RemotingExtensionInstaller` を ActorSystem config に渡し、最後は `ActorSystem::terminate` だけを呼ぶ。

`remote()` は診断・内部テスト用に残す余地はあるが、startup API として docs / showcase / public surface test に出してはならない。`spawn_run_task()` と `shutdown_and_join()` も通常利用 path では直接呼ばせない。

### Decision 5: `RemoteRouteeExpansion` の public 必要性を再評価する

`RemoteRouteeExpansion` は remote router の構築 helper だが、現在は `StdRemoteActorRefProvider` を手動で作って渡す前提になっている。これは remote routee を使う利用者に provider wiring を見せてしまう。

実装では以下の順で整理する。

1. `RemoteRouteeExpansion` が `StdRemoteActorRefProvider::new` を必要としない public 経路に変えられるか確認する。
2. できない場合は `RemoteRouteeExpansion` を `pub(crate)` に落とし、routee expansion は actor system / provider installer の内部責務にする。
3. showcase は public API 経由に更新し、低レベル provider 構築を例示しない。

### Decision 6: tests は公開契約テストと内部テストに分ける

crate 外 integration test は public API のみを使う。内部 runtime driver の細かいテストは `src/**/tests.rs` に残し、private module / `pub(crate)` item を直接検証する。

公開契約テストでは以下を確認する。

- hidden internal 型を external crate から import できない。
- `TcpRemoteTransport` と `RemotingExtensionInstaller` は external crate から利用できる。
- remote lifecycle showcase は `installer.remote()` / `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を呼ばない。
- routee / provider の showcase は低レベル constructor を参照しない。

## Alternatives Considered

### Alternative A: `StdRemoteActorRefProvider` を削除する

却下。これは core wrapper ではなく、actor-core の provider extension point へ remote を接続する adapter bridge である。削除すると local / remote dispatch の責務が別の場所へ漏れるだけで、認知負荷は下がらない。

### Alternative B: すべての runtime 部品を public のまま docs で internal と説明する

却下。Rust の public API は互換性と利用可能性を意味するため、docs だけで internal と説明しても利用者の認知負荷は下がらない。正式リリース前なので、公開範囲を破壊的に縮小する。

### Alternative C: `TcpRemoteTransport` も完全に隠す

却下。core `RemoteShared` に差し込む具体 transport は adapter 実装としてユーザが設定で選ぶ境界であり、`TcpRemoteTransport` は public に残す価値がある。ただし内部 TCP client/server/frame は隠す。

## Risks / Trade-offs

- public re-export 削除により existing showcase / external tests が壊れる。これは正式リリース前の破壊的変更として許容する。
- `StdRemoteActorRefProvider::new` を隠すと、現在の remote routee expansion showcase は作り替えが必要になる。
- 内部型を隠す過程で public method が private type を返せなくなるため、`TcpRemoteTransport` の inherent method 可視性も同時に見直す必要がある。
- provider installer の設計を厚くしすぎると、新しい wrapper 問題を作る。installer は「配線を隠す」だけに留め、core lifecycle の代替入口にはしない。
- install 時に remote start / run task 起動まで行うと、ActorSystem 作成直後に lifecycle event が発火する。作成後 subscribe する showcase は listen event を取り逃がすため、public showcase は event 観測より config-only startup / remote delivery の契約を示す形へ変える。
- shutdown / join を ActorSystem termination に接続するには async join と既存 terminate API の関係を整理する必要がある。同期 terminate から tokio task join を待てない場合は、ActorSystem lifecycle hook または std adapter 専用 shutdown coordinator を明示する。

## Migration Plan

1. public re-export 一覧を固定し、残す型 / 隠す型をテストで明文化する。
2. `transport::tcp` の public re-export を `TcpRemoteTransport` のみに縮小する。
3. `association` と `watcher_actor` を `pub(crate)` module または internal-only re-export に変更する。
4. `RemoteActorRefSender` を provider module 内部へ隠す。
5. `StdRemoteActorRefProvider::new` を `pub(crate)` 化し、installer / config 経由の構築経路を追加または既存 installer に統合する。
6. `RemotingExtensionInstaller` または ActorSystem lifecycle hook が install 後に `RemoteShared::start` と run task 起動を行うようにする。
7. ActorSystem termination から `RemoteShared::shutdown`、event loop wake、tokio `JoinHandle` の完了観測へ到達する経路を追加する。
8. `RemoteRouteeExpansion` の public 必要性を再評価し、手動 provider 配線が必要なら public API から外す。
9. showcase と public surface tests を public API 経由へ更新し、`installer.remote()` / `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` が user-facing path に出ないことを固定する。
10. `rtk cargo test -p fraktor-remote-adaptor-std-rs` と `rtk ./scripts/ci-check.sh ai all` で確認する。

## Open Questions

- `StdRemoteActorRefProvider` は最終的に public 型として残すべきか、actor system extension に完全に隠すべきか。
- routee expansion は `RemoteRouterConfig` の public API としてどの層に置くべきか。
- `TcpRemoteTransport::connect_peer` / `send_handshake` は public debugging API として残す価値があるか、association runtime 専用に閉じるべきか。
- ActorSystem termination API が async join を待てるか。待てない場合、std adapter の run task join をどの lifecycle hook に接続するか。
