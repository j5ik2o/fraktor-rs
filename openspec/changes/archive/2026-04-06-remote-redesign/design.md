## Context

`fraktor-rs` の remote モジュールは現状以下の構造的問題を抱えている (詳細は `proposal.md` 参照)。

```
modules/
├── actor-core / actor-adaptor-std         ← クレート分割済み
├── cluster-core / cluster-adaptor-std     ← クレート分割済み
├── persistence-core                       ← core のみ (adaptor-std 未整備、別件)
├── stream-core / stream-adaptor-std       ← クレート分割済み
├── utils                                  ← 単一クレート (別件)
└── remote/   (fraktor-remote-rs)          ← ❌ 本 change の対象: 単一クレート + 内部 core/std 分離が崩壊
```

`persistence` と `utils` にも分割未整備があるが、本 change のスコープ外 (それぞれ別の change で対応)。本 change は最も深刻な `remote` のみを対象とする。

`remote` の症状: `modules/remote/src/core/` 配下に `#[cfg(feature = "tokio-transport")]` が **53 箇所** (`grep -rn 'cfg(feature = "tokio-transport")' modules/remote/src/core/` で実測、2026-04-07 時点) 散在し、`core/actor_ref_provider/tokio.rs` のように tokio という単語を core モジュール名に含むファイルまで存在する。`modules/remote/src/core/remoting_extension/control_handle.rs:1-479` の `RemotingControlHandle` (本体 479 行 + `control_handle/tests.rs` 222 行 = 合計 701 行) は god object 化し、Pekko には対応物のない `EndpointTransportBridge` という人工概念で core/adapter 境界を埋めている。

参照実装である Apache Pekko Artery の責務分離は以下のようにクリーンになっている:

```
RemoteTransport (abstract, 113行)        ← 唯一の port
   ↑ extends
ArteryTransport (concrete base, 989行)   ← Artery 共通の I/O loop, association registry
   ↑ extends
ArteryTcpTransport (TCP-specific, 518行) ← TCP 固有実装

並行して:
RemoteActorRefProvider (1つだけ, 784行)  ← provider, RemoteActorRef 生成
Association (per-remote, 1240行)         ← outbound state machine + send queue (1箇所に集約)
RemoteWatcher (342行)                    ← death watch + heartbeat
PhiAccrualFailureDetector (295行)        ← 純粋計算
```

本 change では、Pekko の責務分離を Rust + no_std + `&mut self` ベースで再実装した新クレート `fraktor-remote-core-rs` を `modules/remote-core/` に新設するところから開始し、続いて `fraktor-remote-adaptor-std-rs` の実装、依存元切り替え、旧 `modules/remote/` の削除までを一貫して扱う。

## Goals / Non-Goals

**Goals:**

1. **完全な no_std 化**: 新クレートは `std`・`tokio`・`async`・`#[cfg(feature)]` 機能ゲート (no_std 制約と無関係なもの) を一切持たない
2. **Pekko Artery 責務分離の踏襲 (L1 互換)**: 命名・責務境界・状態機械構造が Pekko Artery と一致し、参照実装からの理解可能性を最大化する
3. **god object の解体**: `RemotingControlHandle` (479行) の責務を `Remoting` trait + `Association` + `RemoteWatcher` + `flight recorder` に分散
4. **provider 単一化**: 現状の `loopback / remote / tokio` 3兄弟 provider を **1つの `RemoteActorRefProvider` trait** に統合
5. **`Association` モデルの導入**: 現状の `EndpointWriter` + `EndpointAssociation` + `EndpointTransportBridge` の3分割を、Pekko `Association` 相当の **1つの状態機械** に集約
6. **`&mut self` 原則の徹底**: 内部可変性 (`SpinSyncMutex` + `&self`) は core 内では使用せず、共有が必要な箇所は adapter 側で `AShared` パターンを使う前提とする
7. **時刻の純関数化**: `Instant::now()` を core では呼ばず、時刻入力はすべて引数で受け取る
8. **L2 wire 互換へのアップグレードパス確保**: `Codec` trait で wire format を差し替え可能にし、将来 Pekko Artery TCP wire 互換 codec を追加できる構造にする

**Non-Goals:**

- L2 (Pekko Artery wire 互換) の達成 → 別途
- L3 (Pekko Cluster Sharding 完全互換) の達成 → 別途
- Pekko の過剰機能 (`OutboundCompressionAccess`, `LargeMessageDestinations` 等) の取り込み → 必要になったら後続 change で追加
- remote 再設計と無関係な他モジュール (`persistence`, `utils`) の構造是正 → 別件

## Decisions

### Decision 1: クレート分割への移行 (内部 core/std 分離の廃止)

**選択**: 新クレート `fraktor-remote-core-rs` を作成し、`modules/remote/src/core/` の内部分離方式を捨てる。

**Alternatives considered:**
- *A: 単一クレートのまま `core/` ディレクトリを綺麗にする* → 物理的な強制力がない。`cfg_std_forbid` lint をすり抜ける形で `tokio-transport` feature が混入した過去がある。人間規律だけでは再発する。
- *B: クレート分割* (採用) → 他5モジュールと一貫し、依存関係が物理的な壁になる。`#[cfg(feature = "tokio-transport")]` を完全に排除できる。

**Rationale**: クレート境界がない限り「core が adapter のものを抱え込む」事故は再発する。他モジュールが既に分割しているのだから、remote だけ例外にする理由がない。

### Decision 2: Pekko 互換レベル L1 (設計互換のみ、wire format 独自)

**選択**: 責務分離・命名・概念モデルは Pekko Artery を踏襲。wire format は独自設計とし、`prost`/protobuf 依存を持ち込まない。

**Alternatives considered:**
- *L1 (採用)*: 設計互換のみ、wire format 自由
- *L2*: Pekko Artery TCP wire 互換 (Pekko ノードと相互通信可能) → `Codecs.scala` 857行の完全再実装が必要、`prost` 依存が core を汚す可能性、ActorRef compression dictionary 等の周辺装置も必要
- *L3*: Cluster Sharding まで含めた完全互換 → スコープ爆発

**Rationale**: 「Pekko ノードと混在クラスタを組む」という実利は現時点では存在しない。設計の理解可能性が L1 で十分得られる。L2 へのアップグレードパスは `Codec` trait 差し替えで残せるため将来追加可能。

### Decision 3: Provider 単一化 + remote 専用化 + loopback 短絡の adapter 責務化

**選択**: `RemoteActorRefProvider` trait を1つだけ定義するが、**remote 経路に限定した責務**に絞る。`actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError>` を返し、local path の振り分けは呼び出し側 (Phase B adapter) の責務とする。loopback 短絡 (同一 `UniqueAddress` への配送) は adapter が path を検査して actor-core の local actor ref provider (`LocalActorRefProvider` または `ActorRefProviderShared<LocalActorRefProvider>`) に委譲する形で実装する。

**Alternatives considered:**

- *A: 現状維持 (3 provider)* — Pekko には対応物がなく、責務が不明瞭。Shared trait での実装共有も冗長。**却下**。

- *B: 単一 provider (Pekko unified)* — Pekko の `RemoteActorRefProvider extends LocalActorRefProvider` を踏襲し、core provider が local / remote 両方を解決。**却下の理由**: core は `fraktor-actor-core-rs` の local provider を保持しないため、core 内で local ActorRef を構築する手段がない。spec 要件に「loopback 短絡でローカル ActorRef を返す」と書くと、Phase A では実装不能な責務の先食いになる。

- *C: 単一 provider (remote 専用) + adapter が loopback 振り分け* (**採用**) — core の `RemoteActorRefProvider` は remote path のみを扱い、`RemoteActorRef` を返す。loopback 振り分けは Phase B adapter の責務として明示的に切り出す。actor-core の local provider は adapter 側で扱われ、core には入ってこない。

- *D: sum type `ResolvedActorRef`* — `enum ResolvedActorRef { Local(ActorRef), Remote(RemoteActorRef) }` を返す。**却下の理由**: Pekko には存在しない型が増え、呼び出し側に常に2分岐を強いる。C案のほうが責務境界がクリア。

**Rationale**:
- core の責務を「remote 経路の純粋ロジック」に限定でき、Phase A で **実装可能** になる
- Pekko の unified provider パターンを無理に写像せず、Rust の責務分離に合わせる
- loopback 振り分けは adapter の transport dispatch と同じレイヤで行えるので自然
- 前回のレビューで指摘された「loopback フォールバックで local ActorRef を返すと core spec に書きながら、core は local provider を持たない」という自己矛盾を構造的に解消する

**影響範囲**:
- `RemoteActorRefProvider::actor_ref` の戻り値は `Result<RemoteActorRef, ProviderError>`
- `RemoteActorRef` は data-only 型として公開される (`path` + `remote_node_id` + accessor のみ)
- core spec には「loopback 短絡 → local ActorRef 返却」の Requirement を持たせない
- Phase B adapter で `(a) authority なし path は local 分岐、(b) authority あり + Address 一致かつ uid は path 側が 0 のとき wildcard / non-zero のときのみ比較、なら local 等価 path に正規化して local 分岐、(c) authority あり + 上記条件を満たさない場合は remote 分岐` という 3 分岐で振り分けを実装する
- adapter 側 `StdRemoteActorRefProvider` は **adapter 専用エラー型** (`StdRemoteActorRefProviderError`) を持ち、core `ProviderError` と actor-core `ActorError` をラップする
- loopback 用の独立 transport (`LoopbackTransport`) は core では作らない (test-support を除く)

### Decision 4: Association を単一の状態機械に集約

**選択**: 現状の `EndpointWriter` + `EndpointAssociation` + `EndpointTransportBridge` の3分割を、Pekko `Association` 相当の **1つの `&mut self` 状態機械** に統合する。`Association` は core の純粋ロジックとして配置し、I/O loop は Phase B で adapter 側に置く。

**Alternatives considered:**
- *A: 現状維持* → 責務が3箇所に散らばり、Pekko の `Association` と概念対応が取れない。`EndpointTransportBridge` という Pekko に存在しない概念が必要になる。
- *B: 単一 Association* (採用) → Pekko と一致し、`EndpointTransportBridge` 概念が不要になる。

**State machine の状態 (proposal の Capabilities と一致):**

```
                  ┌─────────────┐
                  │    Idle     │ ← 初期状態
                  └──────┬──────┘
                         │ associate(endpoint)
                         ▼
                  ┌─────────────┐
                  │ Handshaking │
                  └──┬───────┬──┘
      handshake_     │       │ handshake_timed_out
      accepted       │       ▼
                     │    ┌─────────────┐
                     │    │    Gated    │
                     │    └──────┬──────┘
                     ▼           │
             ┌─────────────┐     │
             │   Active    │     │
             └──────┬──────┘     │
          quarantine│             │
          (reason)  │             │
                    ▼             │
             ┌──────────────┐    │
             │ Quarantined  │    │
             └──────┬───────┘    │
                    │             │
                    └──────┬──────┘
                           │
                    recover(endpoint)                recover(None)
                           │                                 │
                           ▼                                 ▼
                    ┌─────────────┐                   ┌─────────────┐
                    │ Handshaking │                   │    Idle     │
                    └─────────────┘                   └─────────────┘

遷移規則:
- associate(endpoint, now)        : Idle → Handshaking
- handshake_accepted(node, now)   : Handshaking → Active (+ deferred flush)
- handshake_timed_out(now, until) : Handshaking → Gated (+ deferred discard)
- quarantine(reason, now)         : Active/Handshaking/Gated → Quarantined (+ deferred discard)
- gate(resume_at, now)            : Active → Gated
- recover(Some(endpoint), now)    : Gated/Quarantined → Handshaking (+ StartHandshake effect)
- recover(None, now)              : Gated/Quarantined → Idle (endpoint 未指定時)
```

**Rationale**: 既存 `EndpointAssociationCoordinator` は既に command/effect 分離された純粋な状態機械なので、Pekko `Association` のシェイプに合うように再構成すれば移植コストは比較的低い。

### Decision 5: god object `RemotingControlHandle` の責務分散

**選択**: 479行の `RemotingControlHandle` を以下に解体する:

| 現状の責務 | 新しい配置 | クレート |
|---|---|---|
| lifecycle state | `extension::lifecycle_state` | core |
| transport_ref 保持 | `StdRemoting` 内部 | adapter (Phase B) |
| writer/reader/bridge_factory/endpoint_bridge | `association_runtime/` | adapter (Phase B) |
| watcher_daemon (actor) | `watcher_actor/` | adapter (Phase B) |
| heartbeat_channels | `tcp_transport/connection.rs` | adapter (Phase B) |
| flight recorder | `instrument/flight_recorder` | core |
| backpressure listener | trait は core、配信は adapter | core + adapter |
| snapshots (data 型) | `RemoteAuthoritySnapshot` | core |
| canonical_host / canonical_port | `RemoteSettings` | core |
| heartbeat 送信 (実 I/O) | `tcp_transport` | adapter (Phase B) |

**Rationale**: god object の各責務はそれぞれ自然な置き場が決まる。core に残るのは「純粋な状態と data 型」だけ。

### Decision 6: 内部可変性禁止と AShared パターン

**選択**: 新クレートの core 内では `SpinSyncMutex<T>` + `&self` 操作を使わず、**すべて `&mut self` で書く**。共有が必要な箇所 (例: `AssociationRegistry` から複数箇所が同じ `Association` を参照する) では adapter 側で `AssociationShared = AShared<Association>` のような薄いラッパーを定義する (Phase B)。

**Alternatives considered:**
- *A: core 内で `SpinSyncMutex` を使う* → `.agents/rules/rust/immutability-policy.md` 違反。Rust の借用システムの価値が失われる。
- *B: `&mut self` のみ* (採用) → Rust の借用システムを活かせる。adapter 側で薄くラップする方が責務分離が綺麗。

**Rationale**: 既存ルール (`immutability-policy.md`) と完全に一致する。core が純粋な `&mut self` ロジックなら adapter からのテストも書きやすい。

### Decision 7: 時刻入力の純関数化

**選択**: core では `core::time::Instant` / `std::time::Instant` / `tokio::time::Instant` を直接呼ばない。時刻は `now: u64` (millis since epoch or monotonic) を引数で受け取る純関数にする。

**Alternatives considered:**
- *A: `core::time::Instant`* → `core` (Rust の core, std とは別) に依存できないため不可
- *B: trait `Clock`* → 過剰抽象化。引数で渡す方がテストしやすい
- *C: `now: u64` 引数* (採用) → Pekko は `System.nanoTime()` を直接呼ぶが、Rust では adapter で `Instant::now()` を呼んで core に渡せばよい

**Rationale**: `failure_detector`、`association` の handshake timeout、`watcher` の heartbeat 判定すべてに時刻が必要だが、全部引数で渡せばテストが純粋関数になる。

### Decision 8: wire format に独自 binary を採用 (protobuf 不採用)

**選択**: `bytes::Bytes` / `BytesMut` ベースの独自 length-prefixed binary format。`Codec` trait で encode/decode を抽象化し、L2 wire 互換 codec を後で差し替え可能にする。

**Alternatives considered:**
- *A: protobuf (`prost`)* → core に重い依存が入る。L1 互換だけなら不要
- *B: 独自 binary* (採用) → 軽量、L1 で十分
- *C: bincode* → 後方互換性のフォーマット安定性が弱い

**Rationale**: L1 では Pekko ノードと相互運用しないので、protobuf を選ぶ理由がない。`Codec` trait で抽象化しておけば L2 移行時に Artery TCP wire 互換 codec を追加できる。

### Decision 9: `RemoteWatcher` の actor 化を adapter に委ねる

**選択**: `RemoteWatcher` の **状態部のみ** を core に置く (誰が誰を watch しているか、最後の heartbeat 時刻、quarantine 判定)。actor 化・スケジューリング・heartbeat 送信は Phase B で adapter (`watcher_actor/`) で行う。

**Alternatives considered:**
- *A: actor として core に置く* → core が actor framework に依存することになり、no_std 制約と相性が悪い
- *B: 状態部のみ core* (採用) → 純粋な input/output 関数として書ける。adapter が actor で wrap

**Rationale**: Pekko の `RemoteWatcher` も Akka actor だが、状態管理ロジックと actor lifecycle は実質的に分離可能。Rust では完全に分離した方が core が綺麗になる。

### Decision 10: SendQueue priority のロジック分離

**選択**: priority queue (system / user) のロジック自体は core (`association/send_queue.rs`)。実 channel (mpsc, bounded queue) は adapter 側 (Phase B)。

**Rationale**: 「system 優先で取り出す」「user は backpressure で paused 可能」というポリシーは core の関心事。実 channel 実装は async runtime 依存なので adapter。既存 `EndpointWriter` (193行) のロジックがほぼそのまま流用できる。

### Decision 11: Settings は型付き struct (HOCON 不採用)

**選択**: `RemoteSettings` を Rust struct で定義し、`new()` コンストラクタ + builder pattern で構築する。HOCON / config crate は使わない。

**Rationale**: Pekko は HOCON だが、Rust では型付き struct の方が安全。`RemoteSettings::new()` に最小必須項目を渡し、optional 項目は `with_*()` で chain する。

### Decision 12: flight recorder の容量実装

**選択**: `Vec<T>` ベースの ring buffer (no_std + alloc 前提)。容量は `RemoteSettings::flight_recorder_capacity` で指定。`heapless` は使わない (alloc が使えるため)。

**Rationale**: 既存 `RemotingFlightRecorder` がほぼ同じ実装。`heapless::Vec` を使う必要がない (no_std だが alloc は使える環境)。

### Decision 13: `UniqueAddress.uid` 型を `u64` にする

**選択**: `UniqueAddress.uid: u64`。`NonZeroU64` は採用しない。

**Alternatives considered:**
- *A: `u64`* (採用) — Pekko の `Long` と同等、`0` を sentinel value として使うことも可能、`Option<UniqueAddress>` との size overhead なし
- *B: `NonZeroU64`* → uid=0 を表現できないため Pekko との概念互換性が低下。また `Option<NonZeroU64>` にすると逆に size overhead を発生させる場面がある

**Rationale**: Pekko の `Long` (符号付き 64bit だが実質的に非負で使用) に最も近い。uid=0 は「uid 未確定」のマーカーとして運用可能で、handshake 完了前の状態表現にも使える。

### Decision 14: `EventPublisher` は `ActorSystemWeak` を直接保持する (現状方針の踏襲)

**選択**: `EventPublisher` は独自の trait abstraction (`LifecycleEventSink` 等) を経由せず、`fraktor_actor_core_rs::core::kernel::system::ActorSystemWeak` を直接フィールドとして保持する。

**前提**: この方針は **既存 `modules/remote/src/core/event_publisher.rs:18-19` で既に採用されている** (`pub struct EventPublisher { system: ActorSystemWeak }`)。本 Decision は新たな方針の提案ではなく、新クレートでも同じパターンを踏襲することの明示的追認である。

**Alternatives considered:**
- *A: 独自 `LifecycleEventSink` trait で抽象化* — core は既に `fraktor-actor-core-rs` に依存しており、追加抽象化は過剰。将来 actor-core から独立させたくなったら、その時点で trait 化すればよい。**却下**
- *B: `ActorSystemWeak` を直接保持* (**採用、現状踏襲**) — 既存 event_publisher.rs と同じ構造。移植コストが最小で、責務分離も明確

**Rationale**:
- 既存 `modules/remote/src/core/event_publisher.rs` が既にこのパターン。新クレートで別の抽象層を挟むと移植コストが増え、かつ既存テストやコールサイトの知見が活かせない
- core クレートは最初から `fraktor-actor-core-rs` に依存する前提であり、`ActorSystemWeak` への直接依存は責務違反にならない
- 将来 actor-core から独立させたくなった場合は、その時点で trait 化する選択肢が残る

### Decision 16: `RemotingLifecycleEvent` は actor-core 既存型を再利用する (remote-core で新設しない)

**選択**: `remote-core` は独自の `RemotingLifecycleEvent` enum を定義せず、既存の `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent` を直接参照する。`EventPublisher`・`AssociationEffect::PublishLifecycle` 等のフィールド/引数型はすべて actor-core の型を使う。

**Alternatives considered:**
- *A: remote-core 独自 enum を新設し、actor-core 型への変換関数を提供* → 型が二重化し、バリアント追加時に両方を更新する必要がある。ドリフトのリスクが高い
- *B: actor-core 既存型を直接参照* (採用) → 単一の型定義。actor-core の `EventStreamEvent::RemotingLifecycle(event)` バリアントが期待する型と完全一致し、変換不要
- *C: remote-core 側で enum を定義し、actor-core 側から remote-core に依存を逆転させる* → 依存方向が逆転し、actor-core が remote-core を依存することになるため不可

**Rationale**: 
- `actor-core/src/core/kernel/event/stream/event_stream_event.rs:52` が既に `RemotingLifecycle(RemotingLifecycleEvent)` バリアントを持ち、`remoting_lifecycle_event.rs` で型を定義済み
- `remote-core` は `fraktor-actor-core-rs` に依存するため、actor-core の型を直接 `use` できる
- 二重化を避けることで、将来のバリアント追加・フィールド変更時に一箇所だけ更新すればよい
- YAGNI / Less is more 原則に合致

**影響範囲**:
- `modules/remote-core/src/extension/lifecycle_event.rs` は作成しない
- `EventPublisher::publish_lifecycle` の引数型は actor-core の型
- `AssociationEffect::PublishLifecycle` のペイロードは actor-core の型
- `extension` spec (`remote-core-extension`) の関連 Requirement / Scenario は actor-core 型参照を要求する形に修正済み

### Decision 15: 単一 change 化による legacy-code-temporary-usage ルール3 への構造的準拠

**選択**: 本 change は **Phase A〜E の全作業を1つの openspec change** として扱い、`openspec archive remote-redesign` は **全フェーズ完了後に1回だけ** 実行する。Phase ごとの中間 archive は行わない。

**Alternatives considered:**

- *A: 現状維持 (5 phase 分割 + 各 phase 個別 archive)* — `remote-core-bootstrap` を Phase 1 として archive し、Phase 2 以降は別 change にする **旧案**。archive 時点で旧 `modules/remote/` が残っているため `legacy-code-temporary-usage.md` ルール3「PRまたはタスク完了時には、同一責務のレガシー実装を残さないこと」違反。例外承認を毎 change で取得する必要があり、かつ archive された spec が「予測に基づく契約」となり Phase 2+ で spec 修正の change が追加発生する。**却下**。

- *B: 単一 change 化* (**採用**) — `remote-redesign` という1つの change にすべての作業を集約し、最後に1回だけ archive する。archive 時点で旧 `modules/remote/` は完全削除済みのため、ルール3 に構造的に準拠する。例外承認が不要で、spec は実装済み・検証済みの状態で `openspec/specs/` に確定する。

- *C: 垂直スライス分割 (loopback → TCP → watcher 等で slice ごとに archive)* — 理論的には各 archive 時点でルール3 準拠だが、remote subsystem の内部 (Address, Envelope, Association, Watcher, Provider) が密結合しており slice を切り出せない。過渡期の routing layer 自体が legacy-temporary となり別のルール違反を生む。**却下**。

**Rationale**:
- `legacy-code-temporary-usage.md` ルール3 への構造的準拠が最重要。例外承認ベースの運用はルールの原則 (「短期の過渡状態」「完成状態での共存を許容しない」) から外れる
- OpenSpec の `specs/` ディレクトリは「確定した要件」を保持する場所。実装済みの spec を archive することで「予測」ではなく「検証済み契約」となる
- 単一 change = 単一 git PR ではない。本 change は **複数 git PR** で段階的に実装され (Phase A/B/C/D/E)、各 PR は作業単位として独立してレビュー・マージ可能。openspec change の archive は最後の git PR (旧削除完了) と同時に行う

**影響範囲**:
- tasks.md は Phase A (core 実装) から Phase E (旧削除) まで拡張される
- specs/ には remote-core-* だけでなく remote-adaptor-std-* 系の capability も含まれる
- 旧 `modules/remote/` の削除要件は既存 capability の migration 契約として `remote-core-package` spec 内に記述する (新 capability は作らない)
- change の "in progress" 期間は長期化するが、git PR 単位で進捗追跡できるため運用上の問題にはならない

### Decision 17: 複数 git PR による段階実装 (single openspec change, multiple git PRs)

**選択**: `remote-redesign` は単一の openspec change だが、実装は **複数の git PR** に分割する。各 PR は Phase A〜E の作業単位 (または更に細かい sub-unit) に対応し、独立してレビュー・マージ可能とする。openspec change の archive は最後の PR で旧削除と同時に行う。

**各 Phase の PR 区切りの目安**:

| Phase | 内容 | 想定 PR 数 |
|---|---|---|
| Phase A | core 実装 (Section 1-16 相当) | 3-5 PR (クレート骨格 / wire+envelope / association / watcher+failure_detector / provider+extension 等で分割) |
| Phase B | adapter 実装 (tcp_transport, runtime, provider impl, extension installer) | 3-4 PR |
| Phase C | テスト移植 | 1-2 PR |
| Phase D | 依存元切り替え (cluster-adaptor-std 等) | 1-2 PR |
| Phase E | 旧 `modules/remote/` 削除 + workspace 更新 + archive | 1 PR |

**Alternatives considered:**
- *A: 1 change = 1 PR (巨大 PR)* → レビュー不能。却下
- *B: 複数 change + 中間 archive* → Decision 15 で却下済み
- *C: 複数 PR per 単一 change* (採用) — openspec と git の役割分離

**Rationale**: openspec change は「仕様の単位」であり、git PR は「レビュー・マージの単位」である。両者は1:N で対応してよい。change の archive は仕様が確定した時点 (= 旧削除と検証完了の時点) に行うのが自然であり、複数 PR での段階実装はそのための標準的な手法。

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| Pekko Artery の概念をそのまま持ち込むと `OutboundCompressionAccess`、`LargeMessageDestinations` 等で過剰になる | **Less is more** 原則に従い、Phase A/B では必要最小限のみ。後続 change で必要に応じて追加 |
| `Association` を `&mut self` で書くと、複数 Association を並行実行する際の調停が adapter で必要になる | Phase B で `AssociationRegistry` (= `BTreeMap<UniqueAddress, AssociationShared>`) として解決。`AssociationShared = ArcShared<SpinSyncMutex<Association>>` で内部可変性は adapter 側に閉じる |
| wire format を独自設計にしたことで、将来 Pekko 互換にしたくなったときに codec 全体を書き直す必要がある | `Codec` trait で encode/decode を抽象化。実装差し替えで対応可能 |
| 既存 `modules/remote/` を Phase E まで温存することで、しばらく **2つの remote 実装が並存** する | `legacy-code-temporary-usage` 原則に従い、Phase E で必ず削除する。並存期間中は新クレートは「未使用」状態 (Phase D で依存元を切り替えるまで) |
| `SendQueue` の priority logic を core に置いても、実 channel が adapter 側のため統合テストが Phase B までできない | core 単体の unit test で `Vec` ベースのキュー操作を検証。adapter 側の channel 統合テストは Phase B で追加 |
| Pekko `Association` の状態機械は1240行あり、全機能を Phase A で再実装するとスコープが膨らむ | Phase A では state machine の **状態と遷移** のみ集中的に実装。`SystemMessageDelivery` の ack-based redelivery 詳細、handshake protocol 詳細、quarantine timeout 自動回復などは Phase A で骨格、Phase B で完成度を上げる |
| 時刻入力を引数で受け取る純関数化は、呼び出し側 (adapter) が必ず正しい時刻を渡す責務を負う | adapter 側で `Instant::now()` を呼ぶ箇所を集約し、レビューで監視 |

## Migration Plan

本 change の migration は Phase A-E で段階的に進む:

1. Phase A で `modules/remote-core/` を新設し、core の純粋ロジックを実装
2. Phase B で `modules/remote-adaptor-std/` を新設し、runtime 配線を実装
3. Phase C で既存統合テストを新クレート群へ移植
4. Phase D で依存元 (`cluster-adaptor-std` 等) を新クレート群に切り替え
5. Phase E で旧 `modules/remote/` を削除し、workspace 参照を掃除して archive

**Rollback**:
- Phase A/B の途中なら、新規クレートと workspace members 追加分を戻せば旧実装に影響なく撤回できる
- Phase D 以降は依存切り替えを伴うため、rollback は新クレート追加の巻き戻しではなく、依存元を旧 `fraktor-remote-rs` に戻す明示的な差し戻しになる

**archive の関係**: openspec change の archive は Phase E 完了時に一度だけ実施する。Phase A-D の成果物はすべてこの最終 archive に向けた中間状態である。

## Open Questions

以下は実装着手初期に確定する:

1. **`Association` の Phase B 共有ラッパーの具体形**: `AssociationShared = ArcShared<SpinSyncMutex<Association>>` を第一候補とするが、`AShared` パターンの命名・公開範囲は実装時に最終確定する
2. **`Codec` trait の粒度**: PDU 種別ごとに個別実装型を作り、`Codec<EnvelopePdu>`・`Codec<HandshakePdu>` のように `Codec<T>` ジェネリック trait を実装する方針で進める。ただし実装開始時に複数 PDU 間で共通ヘルパが必要になれば再検討する
3. **`FlightRecorderEvent` 型の決定**: まず `enum FlightRecorderEvent` を採用 (固定バリアント)。将来ユーザ定義イベントが必要になれば、その時点で `trait FlightRecorderEvent` への移行を検討する (YAGNI)

(以前あった Open Question #4 「uid 型」および Settings builder 方針は、それぞれ Decision 13 および Decision 11 として確定した)
