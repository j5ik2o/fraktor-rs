## Why

`modules/remote/` (= `fraktor-remote-rs` 単一クレート) は Apache Pekko Artery の責務分離モデルから大きく逸脱しており、表面修正では治らない構造的問題を抱えている。

**症状 (測定可能な事実):**

- `modules/remote/src/core/remoting_extension/control_handle.rs:1-479` の `RemotingControlHandle` (本体 479 行 + `control_handle/tests.rs` 222 行) が god object 化し、lifecycle / transport_ref / writer / reader / bridge_factory / watcher_daemon / heartbeat_channels / flight recorder / backpressure listener / snapshots を一手に抱えている
- `modules/remote/src/core/` 配下に `#[cfg(feature = "tokio-transport")]` が **53 箇所** (`grep -rn 'cfg(feature = "tokio-transport")' modules/remote/src/core/`) 散在し、core が tokio 実装の前提に汚染されている
- `modules/remote/src/core/actor_ref_provider/tokio.rs` のように **モジュール名に "tokio" を含むファイル** が core 内に存在する
- `EndpointTransportBridge` という Pekko に対応物のない「core から adapter を呼ぶための人工的な橋渡し」概念まで生まれている
- `RemoteActorRefProvider` 系に `loopback` / `remote` / `tokio` の3兄弟が並存しており、Pekko の単一 provider 原則と矛盾する

**他モジュールとの比較:**

他モジュールは既にクレート分割 (`*-core` + `*-adaptor-std`) 構成への移行を進めている:

| モジュール | 状態 |
|---|---|
| `actor-core` / `actor-adaptor-std` | ✅ 分割済み |
| `cluster-core` / `cluster-adaptor-std` | ✅ 分割済み |
| `stream-core` / `stream-adaptor-std` | ✅ 分割済み |
| `persistence-core` | ⚠ core のみ (adaptor-std 未整備。本件とは別途) |
| `utils` | ⚠ 単一クレート (remote と同様、別件) |
| **`remote`** | ❌ **単一クレート + 内部 core/std 分離が崩壊** |

`persistence` と `utils` にも同様の未整備があるが、本 change のスコープ外。本 change は **最も深刻な remote** のみを対象とする。

**なぜ今か:**

- 本リポジトリは正式リリース前 (CLAUDE.md に「破壊的変更を歓迎」「リリース状況: まだ正式リリース前の開発フェーズ」と明記)
- remote の責務モデルが不安定なまま cluster 機能追加や persistence 連携を進めると、後続モジュールの依存先まで再設計が波及する
- god object を放置すると、新機能追加のたびに負債が拡大する

## What Changes

本 change は **remote サブシステムの完全再設計と旧実装の置き換え** を **1つの openspec change** として扱い、完了後に1回だけ archive する。これは `legacy-code-temporary-usage.md` ルール3 (「PRまたはタスク完了時には、同一責務のレガシー実装を残さない」) への構造的準拠を担保するための判断である (design.md Decision 15 参照)。

実装は **複数の git PR** に分割され、以下の5つの作業フェーズとして tasks.md 内で整理される:

### Phase A: remote-core クレートの新設と実装

- **新クレート `fraktor-remote-core-rs`** を `modules/remote-core/` に新設 (no_std, no tokio, alloc 前提)
- Pekko Artery の責務分離を Rust + `&mut self` ベースで再実装:
  - `address/` に `Address`・`UniqueAddress`・`RemoteNodeId`・`ActorPathScheme` (Pekko `Address` 互換: protocol-scheme + host + port + system + uid)
  - `wire/` に独自 binary format の codec (envelope/handshake/control/ack PDU、protobuf 不採用、L1 互換)
  - `envelope/` に `InboundEnvelope` / `OutboundEnvelope` (immutable data)
  - `association/` に Pekko `Association` 相当の **per-remote 状態機械** を `&mut self` で実装 (5状態の閉じた遷移、全メソッドが `Vec<AssociationEffect>` を返す)
  - `failure_detector/` に Phi Accrual (純粋計算、時刻は monotonic millis を引数で受け取る)
  - `watcher/` に RemoteWatcher の **状態部のみ** (no scheduler / no actor / no async)
  - `instrument/` に `RemoteInstrument` trait と flight recorder (transport 非依存)
  - `transport/` に **唯一の port** `RemoteTransport` trait (Pekko `RemoteTransport` 互換 API)
  - `provider/` に **remote 専用** の `RemoteActorRefProvider` trait (Decision 3-C: loopback 短絡は Phase B adapter 責務)
  - `extension/` に `Remoting` trait と `RemotingLifecycleState` (5状態の閉じた状態機械: Pending → Starting → Running → ShuttingDown → Shutdown)
  - `settings/` に `RemoteSettings` (型付き struct + builder)

### Phase B: remote-adaptor-std クレートの新設と実装

- **新クレート `fraktor-remote-adaptor-std-rs`** を `modules/remote-adaptor-std/` に新設 (std + tokio 前提)
- Phase A の core port を adapter で実装:
  - `tcp_transport/` に Pekko Artery TCP 相当の実装 (server.rs / client.rs / frame_codec.rs / tcp_transport.rs)
  - `association_runtime/` に Association を駆動する tokio task 群 (outbound loop / inbound dispatch / handshake driver / send queue 統合)
  - `watcher_actor/` に `WatcherState` を actor として駆動する層 (tokio timer + ActorRef messaging)
  - `provider/` に `RemoteActorRefProvider` の実装。adapter が `ActorPath` の authority を検査し、local なら actor-core の local actor ref provider (`LocalActorRefProvider` または `ActorRefProviderShared<LocalActorRefProvider>` 相当)、remote なら core provider を呼ぶ loopback 振り分けロジックを含む
  - `extension_installer/` に actor system への組み込み

### Phase C: 統合テストの移植

- 既存 `modules/remote/tests/` および関連統合テストを新クレートへ移植
- core 単体テストは Phase A、adapter 単体テストは Phase B で作成済み。本 Phase は統合レベルのテスト

### Phase D: 依存元の切り替え

- `modules/cluster-adaptor-std/` 等、現在 `fraktor-remote-rs` に依存しているモジュールを `fraktor-remote-core-rs` + `fraktor-remote-adaptor-std-rs` への依存に切り替える
- この時点で新クレートが実運用経路で検証される

### Phase E: 旧実装の削除と archive

- 旧 `modules/remote/` ディレクトリを完全削除
- ワークスペース `Cargo.toml` から `modules/remote` エントリを削除
- `./scripts/ci-check.sh ai all` で全 build/test 通過確認
- `openspec archive remote-redesign` で本 change を archive

**BREAKING**: 本 change 完了時点で `fraktor-remote-rs` クレートは消滅し、ユーザは `fraktor-remote-core-rs` + `fraktor-remote-adaptor-std-rs` の2クレートに移行する必要がある。ただし本リポジトリは正式リリース前のため破壊的変更は許容される。

**L1 Pekko 互換**: 設計・命名・概念モデルは Pekko Artery 準拠。wire format は独自 binary (protobuf 不採用)。L2 wire 互換 (Pekko ノードと相互通信) へのアップグレードパスは `Codec` trait 差し替えで残す。

## Capabilities

### New Capabilities (Phase A: remote-core)

- `remote-core-package`: 新クレート `fraktor-remote-core-rs` (`modules/remote-core/`) の存在・no_std 制約・依存方針・モジュール構成・公開境界。`std`・`tokio`・`async` への直接依存を持たないこと。**本 capability は旧 `modules/remote/` の削除要件も含む** (Phase E 完了時点で旧ディレクトリが存在しないことを契約とする; 新 capability `remote-legacy-removed` を作らず、本 capability の一部として migration 契約を表現する)
- `remote-core-transport-port`: `RemoteTransport` trait (唯一の port) の API 契約。Pekko `RemoteTransport` 互換のメソッドセット (`start` / `shutdown` / `send` / `quarantine` / `addresses` / `default_address` / `local_address_for_remote`) を `&mut self` ベース・同期 API で定義する
- `remote-core-association-state-machine`: per-remote `Association` の状態機械 (`Idle` / `Handshaking` / `Active` / `Gated` / `Quarantined` の5状態)、handshake protocol state、quarantine reason、deferred envelope queue、`SendQueue` priority (system / user) の純粋ロジック。すべて `&mut self` で時刻入力は monotonic millis 引数。全状態遷移メソッドは `Vec<AssociationEffect>` (または `SmallVec<[_; N]>` 等の連続コンテナ) を返す
- `remote-core-actor-ref-provider`: **remote 専用** の `RemoteActorRefProvider` trait と関連型 (`RemoteActorRef` data 型、`resolve_remote_address` 関数、`watch`/`unwatch` メソッド)。loopback / remote / tokio の3兄弟 provider は廃止。`actor_ref` の戻り値は `Result<RemoteActorRef, ProviderError>`。**loopback 短絡の実装責務は Phase B adapter 側** にあり、adapter が `ActorPath` の authority を検査して local / remote に振り分け、local なら actor-core の local actor ref provider、remote なら core の `RemoteActorRefProvider` を呼ぶ (Decision 3-C)
- `remote-core-failure-detector`: Phi Accrual failure detector の純粋計算実装。時刻入力は **monotonic millis** 引数で渡す純関数として定義し、`Instant::now()` を呼ばない
- `remote-core-watcher-state`: RemoteWatcher の状態部 (誰が誰を watch しているか、最後の heartbeat 時刻、quarantine 判定)。actor / scheduler / async は持たず、入力イベントを受けて effect を返す純関数
- `remote-core-instrument`: `RemoteInstrument` trait (transport 非依存) と flight recorder (`VecDeque<FlightRecorderEvent>` ベースの ring buffer、no_std + alloc)
- `remote-core-wire-format`: 独自 binary wire format の codec。frame header (length(u32 BE) + version(u8) + kind(u8)) + プリミティブ型の BE 表現 (u8/u16/u32/u64/String as `u32 length + UTF-8 bytes`/Option/bool) + 各 PDU (Envelope/HandshakeReq/HandshakeRsp/Control/Ack) の binary レイアウトを完全定義。`Codec` trait で L2 Pekko wire 互換へのアップグレードパスを確保
- `remote-core-extension`: `Remoting` trait と `RemotingLifecycleState` (5状態の閉じた遷移: Pending → Starting → Running → ShuttingDown → Shutdown)、`EventPublisher` (`ActorSystemWeak` 直接保持)、`RemoteAuthoritySnapshot`、`RemotingError`。god object `RemotingControlHandle` の純粋 lifecycle 責務のみ。`RemotingLifecycleEvent` は新設せず、既存の `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent` を再利用 (Decision 16)
- `remote-core-settings`: `RemoteSettings` 型付き struct と `with_*` builder API。必須項目 (`canonical_host`・`canonical_port`・`handshake_timeout`・`shutdown_flush_timeout`・`flight_recorder_capacity`) のみを Phase A で定義し、ack-based redelivery 関連設定 (`ack_send_window` 等) は Phase B で必要になった時点で追加する (Phase A での先食いを避ける)

### New Capabilities (Phase B: remote-adaptor-std)

- `remote-adaptor-std-package`: 新クレート `fraktor-remote-adaptor-std-rs` (`modules/remote-adaptor-std/`) の存在・依存方針 (std + tokio 許可)・モジュール構成・公開境界
- `remote-adaptor-std-tcp-transport`: Pekko Artery TCP 相当の実装。`RemoteTransport` trait (core port) を `TcpRemoteTransport` として実装し、tokio `TcpListener`/`TcpStream` を利用した bind / accept / connect / frame reading / frame writing を提供
- `remote-adaptor-std-runtime`: `Association` を駆動する tokio task 群 (outbound loop / inbound dispatch / handshake driver)。core の `Association::enqueue`/`next_outbound`/`apply_backpressure`/`handshake_accepted`/`handshake_timed_out`/`quarantine`/`gate`/`recover` をタスク内で呼び出し、effect 列を副作用として実行
- `remote-adaptor-std-provider-dispatch`: `RemoteActorRefProvider` の adapter 側実装。`ActorPath` の authority がない通常 local path はそのまま local actor ref provider に委譲し、authority がある場合は Address 一致を基本に local 判定する。uid は path 側が `0` のとき wildcard として扱い、non-zero のときだけ local uid と比較する。一致時は authority を剥がした local 等価 path に正規化して local provider に委譲し、不一致時は core の `RemoteActorRefProvider` を呼んで `RemoteActorRef` を受け取り、remote 用 `ActorRefSender` を構築して actor-core `ActorRef` にラップする。adapter 側は **`StdRemoteActorRefProviderError`** を持ち、core の `ProviderError` と actor-core の `ActorError` をラップする。**loopback 短絡の実装責務を担う層**

### Modified Capabilities

<!-- 本 change は新クレートの新設が主であり、既存 spec ファイルの Requirement を変更するものはない。
     旧 modules/remote/ の削除は remote-core-package capability 内の Requirement として表現する (新 capability を別途作らない)。 -->

## Impact

### 新規ファイル
- `modules/remote-core/` 配下のすべて
- `modules/remote-adaptor-std/` 配下のすべて
- 本 change 完了後: `openspec/specs/remote-core-*/spec.md` × 10 + `openspec/specs/remote-adaptor-std-*/spec.md` × 4

### 変更ファイル
- ワークスペース `Cargo.toml` の `members` に `modules/remote-core`, `modules/remote-adaptor-std` を追加、後に `modules/remote` を削除
- `modules/cluster-adaptor-std/Cargo.toml` 等、`fraktor-remote-rs` 依存を持つクレートの依存切り替え

### 削除ファイル
- `modules/remote/` ディレクトリ配下のすべて (Phase E)

### 依存元
- `modules/cluster-adaptor-std/` 等が現在 `fraktor-remote-rs` を使用。Phase D で依存切り替え、Phase E で旧クレート削除

### CI
- 新クレート単独で `cargo build` / `cargo test` / `cargo build --no-default-features` (no_std build) が通ること
- Phase E 完了時点で既存 `./scripts/ci-check.sh ai all` が全通過すること

### Lint
- `cfg_std_forbid`、`type-per-file`、`module-wiring`、`ambiguous-suffix` 等の既存 dylint が新クレートにそのまま適用される

### 後方互換性
- **なし**。`fraktor-remote-rs` の公開 API は完全に消滅する (正式リリース前のため許容)
