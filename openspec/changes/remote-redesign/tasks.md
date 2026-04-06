## 0. 品質ゲートの実行モデル (全セクション共通)

品質ゲート task が配置されている各実装セクションでは、実装エージェント (implementing agent) とレビューエージェント (review agent) の責務を分離し、人間オペレータが両者を仲介することで、セクション単位の独立検証を担保する。

**本セクションを読まずに以降のセクションを着手してはならない。**

- [x] 0.1 実装エージェントは本セクション全体を読み、品質ゲートプロトコルに従うことを宣言する (実装着手前の必須ステップ)

### 0.A 役割と実行主体

| 役割 | 担当 | 説明 |
|---|---|---|
| 実装エージェント | 現在 `/opsx:apply` を実行中の AI | tasks.md のサブタスクを順次実装する。品質ゲート task を自動で完了にしてはならない |
| レビューエージェント | 別ターミナルで人間が起動する独立 AI | 実装済みセクションを spec / design と照合してレビュー。実装エージェントとは **別モデル・別プロセス・別コンテキスト** が望ましい |
| 人間オペレータ | ユーザ | セクション完了報告を受けてレビューエージェントを起動し、レビュー結果を実装エージェントに伝える仲介者 |

**推奨される レビューエージェント:**
- Codex CLI (`/codex:review` 相当) — 別モデルが使える点で最も独立性が高い
- 新規 Claude Code セッション (`/clear` 後に本リポジトリで起動) — 別コンテキストで再実行

**非推奨:**
- 実装エージェント自身が `Agent` tool でサブエージェントを起動する自己レビュー — 同一モデル・近い文脈のため独立性が弱い

### 0.B 実装エージェントの挙動プロトコル (厳守)

品質ゲート task (例: `1.9`、`2.8`、...) に到達した際の挙動を以下に定める。実装エージェントはこれから逸脱してはならない。

1. **Gate task を `in_progress` にする** (チェックボックスは `[ ]` のまま変更しない)
2. **ユーザに対して明示的に停止通知を出す**。通知には以下をすべて含める:
   - セクション X の実装サブタスク (X.1〜X.N-1) がすべて完了したこと
   - 品質ゲート実行を依頼する旨
   - 対象 spec ファイル名 (例: `remote-core-package`)
   - 関連 Decision 番号 (例: Decision 1)
   - 注力すべきレビュー観点の短文 (例: 「Cargo.toml メタデータ・依存・lib.rs 属性・ワークスペース登録」)
   - 人間が 0.C 節のテンプレートを用いてレビューエージェントを起動する必要があること
3. **ユーザの応答を待つ**。以下のいずれかの宣言を受け取るまで次セクションに進まない:
   - `gate X.Y pass` — Gate task を `[x]` にマークし、次セクションへ進む
   - `gate X.Y fail: <details>` — `<details>` をもとに当該セクション内に新サブタスク (例: `7.17 <修正内容>`) を追加し、実装・テストを完了させた後に **再度 Gate task からプロトコルを開始** する
   - `skip gate X.Y: <理由>` — Gate task の task 行末に ` (SKIPPED: <理由>)` を追記した上で `[x]` にマークし、次セクションへ進む
4. **チェックボックスの自律的マーキング禁止**: 実装エージェントは品質ゲート task の `- [ ]` を、上記 3 の人間宣言を経ずに自分の判断で `- [x]` に変更してはならない

### 0.C レビューエージェント起動用テンプレート (人間オペレータ向け)

人間は以下のテンプレートを用いて別ターミナルでレビューエージェントを起動する。`X` はセクション番号、`<spec-name>` と `<Decision N>` は実装エージェントの停止通知から転記する。

```
remote-redesign change のセクション X を独立レビューしてほしい。

対象成果物:
  対象セクションに対応する成果物
  例: modules/remote-core/src/{該当モジュール}/
      modules/remote-adaptor-std/src/{該当モジュール}/
      依存切り替え対象クレート
      旧 modules/remote/ 削除差分

照合対象:
  openspec/changes/remote-redesign/specs/<spec-name>/spec.md
  openspec/changes/remote-redesign/design.md (Decision <N>)
  openspec/changes/remote-redesign/tasks.md (セクション X のサブタスク)

チェック観点:
  - spec の各 Requirement / Scenario が実装に反映されているか
  - design.md の Decision が踏襲されているか
  - lints/ 配下の dylint ルール全体を逸脱していないか
  - .claude/rules/rust/ および .agents/rules/rust/ 配下のプロジェクトルール
    (immutability-policy, cqs-principle, type-organization,
    naming-conventions, reference-implementation, module-structure 等)
    に違反していないか

出力:
  1. 発見した issue の一覧 (severity: Critical / Significant / Minor)
  2. 各 issue に対する修正提案 (ファイル:行 を含めて具体的に)
  3. セクション全体としての合否判定 (Pass / Fail)
```

### 0.D 合否判定基準

| 判定 | 条件 | 実装エージェントの次のアクション |
|---|---|---|
| **Pass** | issue なし、または Minor のみ | `gate X.Y pass` 宣言を受けて `[x]` にマーク、次セクションへ進む。Minor issue は後続セクションの中で吸収する |
| **Fail** | Critical または Significant を含む | `gate X.Y fail: <details>` 宣言を受けて新サブタスクを追加、修正後に再度 Gate プロトコルを起動 |
| **Skip** | 人間が急ぎ等の理由で独立レビューを省略 | `skip gate X.Y: <理由>` 宣言を受けて `(SKIPPED: <理由>)` 追記 + `[x]`、次セクションへ進む |

**いかなる場合でも、人間からの明示的宣言なしに次セクションの作業を開始してはならない。**

### 0.E Rationale (なぜ人間仲介の独立レビューが必要か)

- **真の独立性の確保**: 同一セッションのサブエージェントは context・モデルが共通で、実装時の思い込みを引き継ぎやすい。別ターミナル・別モデル (または別セッション) で実行することで、独立した視点が得られる
- **スコープ暴走の早期検知**: セクション単位で人間がレビュー結果を見ることで、Phase A 全体を実装してから問題に気づくリスクを避ける
- **AI エージェント間の認識ズレ監視**: 実装エージェントとレビューエージェントが別モデル/プロセスであれば、モデル固有のバイアスに引きずられにくい
- **コスト観点**: Codex CLI 等は利用制限/有料枠あり。多数の独立レビューはトークン消費が少なくない。急ぎの日は `skip gate` 機構を活用してよい (ただし skip を連発すると change 完了時に問題が集中するので、重要なゲート — 特に Section 7 association、Section 13 extension、Section 19 runtime、Section 27 依存切り替え、Section 29 旧削除 — は可能な限り実行する)

---

# Phase A: remote-core クレートの新設と実装

Phase A は新クレート `fraktor-remote-core-rs` の骨格作成と核となる純粋ロジックの実装を担う。完了時点で新クレートは **未使用** だが、Phase B 以降で依存される前提の全 trait・データ型・state machine が揃う。Phase A 単体では archive せず、Phase E 完了時に全 change として archive する。

## 1. クレート骨格の作成

- [x] 1.1 `modules/remote-core/` ディレクトリを新規作成する
- [x] 1.2 `modules/remote-core/Cargo.toml` を作成する (`name = "fraktor-remote-core-rs"`、`edition = "2024"`、ライセンス・description・homepage・repository・documentation・keywords・categories を他モジュールと統一する)
- [x] 1.3 `[dependencies]` には **最小限の依存のみ** を追加する: `fraktor-actor-core-rs`、`fraktor-utils-rs`、`bytes`。他クレート (`portable-atomic`、`hashbrown`、`ahash`、`spin` 等) は各サブモジュールの実装時に必要性が確認されてから追加する (`tokio`・`async-std`・`futures`・`async-trait`・`tokio_util`・`prost` は常に追加しない)
- [x] 1.4 `[features]` に `default = []`、`test-support = []` のみ定義する。`std`・`tokio-transport` 等の transport 実装ゲートは追加しない
- [x] 1.5 `[lints] workspace = true` を設定する
- [x] 1.6 ワークスペースルートの `Cargo.toml` の `members` に `modules/remote-core` を追加する
- [x] 1.7 `modules/remote-core/src/lib.rs` を **既存 `modules/cluster-core/src/lib.rs:1-51` のパターンに揃えて** 作成する。必須属性リスト (順序含めて既存と一致): `#![deny(missing_docs)]`、`#![deny(unsafe_op_in_unsafe_fn)]`、`#![deny(unreachable_pub)]`、`#![allow(unknown_lints)]` (dylint `cfg_std_forbid` を素の `cargo build` 環境で unknown lint エラーにしないため必須)、`#![deny(cfg_std_forbid)]`、`#![cfg_attr(not(test), no_std)]` (test では std を使えるように `cfg_attr` で分岐する必要あり — 素の `#![no_std]` は後続の round-trip テストを破壊する)、`extern crate alloc;`、crate-level docstring
- [x] 1.8 `cargo build -p fraktor-remote-core-rs` が成功することを確認する (空モジュール状態でも no_std build できること)
- [x] 1.9 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-package`。関連 Decision: 1。注力観点: `Cargo.toml` メタデータ・依存の最小化、`lib.rs` 属性が既存 `cluster-core` と完全一致していること (特に `#![allow(unknown_lints)]` と `#![cfg_attr(not(test), no_std)]` の存在)、ワークスペース登録

## 2. address モジュール

- [x] 2.1 `src/address.rs` および `src/address/` を作成する
- [x] 2.2 `address/address.rs` に `pub struct Address { host, port, system }` を定義する
- [x] 2.3 `address/unique_address.rs` に `pub struct UniqueAddress { address: Address, uid: u64 }` を定義する (Decision 13 により `uid` は `u64`)
- [x] 2.4 `address/remote_node_id.rs` に `pub struct RemoteNodeId` を定義する (既存 `core/remote_node_id.rs` の責務を移植)
- [x] 2.5 `address/scheme.rs` に `pub enum ActorPathScheme` を定義する (既存 `actor_ref/actor_path` 由来のスキーム表現)
- [x] 2.5a (gate 2.8 fail 追加) 当初 `pub use fraktor_actor_core_rs::...::ActorPathScheme` で再エクスポートしたが、`module_wiring_lint` の `no-parent-reexport` ルール (「再エクスポートは末端モジュールの直属親以外では禁止」) に違反したため、`address/scheme.rs` に **独自 `pub enum ActorPathScheme { Fraktor, FraktorTcp }` を新規定義** し直した。`as_str()` accessor も定義
- [x] 2.6 `Address`・`UniqueAddress` の `Display`、`PartialEq`、`Eq`、`Hash` を実装する
- [x] 2.7 `address` モジュールの unit test を作成する (構築・equality・display)
- [x] 2.8 【品質ゲート】Section 0 のプロトコルに従う。関連 Decision: 13 (`UniqueAddress.uid = u64`)。注力観点: `Display`・`PartialEq`・`Eq`・`Hash` の実装完全性、`ActorPathScheme` 列挙の過不足、1file1type ルール遵守

## 3. settings モジュール

spec: `remote-core-settings`

- [x] 3.1 `src/settings.rs` および `src/settings/` を作成する
- [x] 3.2 `settings/remote_settings.rs` に `pub struct RemoteSettings` を定義する。**Phase A で含めるフィールド**: `canonical_host`・`canonical_port`・`handshake_timeout`・`shutdown_flush_timeout`・`flight_recorder_capacity`。**Phase A では含めないフィールド**: `ack_send_window`・`ack_receive_window` (ack-based redelivery の具体実装は Phase B で adapter 側に配置されるため、設定フィールドも Phase B で必要になった時点で追加する。Phase A で先食いしない)
- [x] 3.3 フィールドは `pub` 修飾子を持たせず、private とする
- [x] 3.4 `RemoteSettings::new(canonical_host: impl Into<String>) -> Self` コンストラクタを実装し、optional 項目はデフォルト値で初期化する
- [x] 3.5 `with_canonical_port`、`with_handshake_timeout`、`with_shutdown_flush_timeout`、`with_flight_recorder_capacity` の builder メソッドを実装する (`self` consume 型、method chain 可能)。`with_ack_*` 系は Phase A では実装しない (Phase B で追加)
- [x] 3.6 accessor `canonical_host(&self) -> &str` 等を実装する
- [x] 3.7 `settings` モジュールの unit test を作成する (デフォルト値・builder chain・元インスタンスの不変性)
- [x] 3.8 `modules/remote-core/src/settings/` 配下で `use std::` が存在しないことを確認する (`core::time::Duration` と `alloc::string::String` のみ使用)
- [x] 3.9 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-settings`。関連 Decision: 11 (builder pattern)。注力観点: フィールド非公開化、`with_*` method chain 動作、デフォルト値の妥当性、`std` 不依存

## 4. wire モジュール

spec: `remote-core-wire-format`

- [x] 4.1 `src/wire.rs` および `src/wire/` を作成する
- [x] 4.2 `wire/wire_error.rs` に `pub enum WireError { InvalidFormat, UnknownVersion, UnknownKind, Truncated, InvalidUtf8, FrameTooLarge }` を定義し、`Debug`・`Display`・`core::error::Error` を実装する
- [x] 4.3 `wire/frame_header.rs` に `pub struct FrameHeader { length: u32, version: u8, kind: u8 }` を定義する (length-prefixed framing: 4 byte big-endian u32 + 1 byte version + 1 byte kind)
- [x] 4.4 `wire/codec.rs` に `pub trait Codec<T>` を定義し、`encode(&self, value: &T, buf: &mut BytesMut) -> Result<(), WireError>` および `decode(&self, buf: &mut Bytes) -> Result<T, WireError>` を宣言する
- [x] 4.5 `wire/envelope_pdu.rs` に `pub struct EnvelopePdu` を定義する (recipient_path、sender_path、payload、correlation_id、priority)
- [x] 4.6 `EnvelopePdu` の `Codec` 実装を作成する (frame header 付き、version byte 付き、length-prefixed) — `wire/envelope_codec.rs` に `EnvelopeCodec` として分離 (1file1type)
- [x] 4.7 `wire/handshake_pdu.rs` に `pub enum HandshakePdu { Req(HandshakeReq), Rsp(HandshakeRsp) }` を定義する (`HandshakeReq` / `HandshakeRsp` は `wire/handshake_req.rs` / `wire/handshake_rsp.rs` に分離)
- [x] 4.8 `HandshakePdu` の `Codec` 実装を作成する — `wire/handshake_codec.rs` に `HandshakeCodec` として分離
- [x] 4.9 `wire/control_pdu.rs` に `pub enum ControlPdu { Heartbeat(...), Quarantine(...), Shutdown(...) }` を定義する
- [x] 4.10 `ControlPdu` の `Codec` 実装を作成する — `wire/control_codec.rs` に `ControlCodec` として分離。subkind 0x00/0x01/0x02 で Heartbeat/Quarantine/Shutdown を識別
- [x] 4.11 `wire/ack_pdu.rs` に `pub struct AckPdu` を定義する (system message ack — sequence_number/cumulative_ack/nack_bitmap)
- [x] 4.12 `AckPdu` の `Codec` 実装を作成する — `wire/ack_codec.rs` に `AckCodec` として分離
- [x] 4.13 各 PDU の round-trip unit test を作成する (encode → decode → 元と一致) — 20 テスト in `wire/tests.rs`
- [x] 4.14 未知 version byte の decode で `WireError::UnknownVersion` が返ることを検証する test を作成する
- [x] 4.15 truncated buffer の decode で `WireError::Truncated` が返ることを検証する test を作成する
- [x] 4.16 length field が buffer 長を超えるフレームの decode で `WireError::InvalidFormat` または `WireError::Truncated` が返ることを検証する test を作成する
- [x] 4.17 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-wire-format`。関連 Decision: 2 (L1 互換)、8 (独自 binary)。注力観点: frame header 形式 (length+version+kind) の一貫性、各 PDU の round-trip 網羅、エラー分岐 (UnknownVersion / UnknownKind / Truncated / InvalidFormat) のテスト網羅、`bytes` ゼロコピー活用度

## 5. envelope モジュール

- [x] 5.1 `src/envelope.rs` および `src/envelope/` を作成する
- [x] 5.2 `envelope/outbound_envelope.rs` に `pub struct OutboundEnvelope { recipient, sender, message, priority, remote_node, correlation_id }` を定義する (immutable data)
- [x] 5.3 `envelope/inbound_envelope.rs` に `pub struct InboundEnvelope { recipient, remote_node, message, sender, correlation_id, priority }` を定義する (immutable data)
- [x] 5.4 `envelope/priority.rs` に `pub enum OutboundPriority { System, User }` を定義する — wire 仕様に合わせ `to_wire`: `System=0, User=1` (旧 remote の `System=1, User=0` から変更)
- [x] 5.5 `envelope` モジュールの unit test を作成する (構築・accessor) — 8 tests
- [x] 5.6 【品質ゲート】Section 0 のプロトコルに従う。注力観点: `OutboundEnvelope`/`InboundEnvelope` の immutable data 化、フィールド過不足、priority enum の過不足、accessor 公開範囲、1file1type ルール遵守

## 6. transport モジュール (port)

spec: `remote-core-transport-port`

- [x] 6.1 `src/transport.rs` および `src/transport/` を作成する
- [x] 6.2 `transport/transport_error.rs` に `pub enum TransportError { UnsupportedScheme, NotAvailable, AlreadyRunning, NotStarted, SendFailed, ConnectionClosed }` を定義し、`Debug`・`Display`・`core::error::Error` を実装する
- [x] 6.3 `transport/transport_endpoint.rs` に `pub struct TransportEndpoint` を定義する (既存からの移植)
- [x] 6.4 `transport/transport_bind.rs` に `pub struct TransportBind` を定義する (既存からの移植)
- [x] 6.5 `transport/remote_transport.rs` に `pub trait RemoteTransport` を定義する (Decision: Pekko 互換 API)
- [x] 6.6 `RemoteTransport` に `start(&mut self)`、`shutdown(&mut self)`、`send(&mut self, envelope: OutboundEnvelope)`、`addresses(&self) -> &[Address]`、`default_address(&self) -> Option<&Address>`、`local_address_for_remote(&self, remote: &Address) -> Option<&Address>`、`quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason)` を宣言する — `QuarantineReason` は Section 7 の 7.2 を先行実装し `modules/remote-core/src/association/quarantine_reason.rs` に配置 (Section 6 の trait がコンパイルできるようにするため)
- [x] 6.7 すべてのメソッドが `async fn` でないこと、戻り値に `Future` を含まないこと、ロックガード型 (`Guard`・`MutexGuard`・`RwLockReadGuard`・`SpinSyncMutexGuard`) を返さないことを確認する — grep で transport/ 配下ゼロ件 (doc コメント内の言及のみ)
- [x] 6.8 `transport/backpressure_signal.rs` に `pub enum BackpressureSignal { Apply, Release }` を定義する。設置理由: 送信経路の制御信号であり transport/association の双方が消費するが、概念の発生源は transport 側 (Phase B で下流からの backpressure を受信して上流に伝播する)
- [x] 6.9 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-transport-port`。参照: Pekko `RemoteTransport.scala` (113行)。注力観点: メソッドセットの Pekko 互換性、`async fn` 不使用、`Future`/ロックガード型の不返却、CQS 原則 (`&self` query / `&mut self` command)

## 7. association モジュール - 状態機械本体

spec: `remote-core-association-state-machine`

- [x] 7.1 `src/association.rs` および `src/association/` を作成する
- [x] 7.2 `association/quarantine_reason.rs` に `pub struct QuarantineReason` を定義する (`new(message)` コンストラクタ、`message(&self) -> &str` accessor) — Section 6 の task 6.6 trait 署名で必要なため Section 6 実装中に先行実装した
- [x] 7.3 `association/association_state.rs` に `pub enum AssociationState { Idle, Handshaking { endpoint, started_at }, Active { remote_node, established_at }, Gated { resume_at }, Quarantined { reason, resume_at } }` を定義する
- [x] 7.4 `AssociationState::is_active(&self) -> bool` および `is_connected(&self) -> bool` 等の query メソッドを実装する (加えて `is_gated` / `is_quarantined` / `is_idle`)
- [x] 7.5 `association/association_effect.rs` に `pub enum AssociationEffect { StartHandshake { endpoint }, SendEnvelopes { envelopes }, DiscardEnvelopes { reason, envelopes }, PublishLifecycle(fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent) }` を定義する。`PublishLifecycle` のペイロードは **actor-core の既存 `RemotingLifecycleEvent`** を直接参照する (Decision 16)
- [x] 7.6 `association/association.rs` に `pub struct Association { state, send_queue, deferred, local, remote }` を定義する
- [x] 7.7 `Association::new(local: UniqueAddress, remote: Address) -> Self` を実装する
- [x] 7.8 `Association::associate(&mut self, endpoint, now) -> Vec<AssociationEffect>` を実装する (Idle → Handshaking 遷移 + StartHandshake effect)
- [x] 7.9 `Association::handshake_accepted(&mut self, remote_node, now) -> Vec<AssociationEffect>` を実装する (Handshaking → Active 遷移 + deferred flush + Lifecycle::Connected publish)
- [x] 7.10 `Association::handshake_timed_out(&mut self, now, resume_at) -> Vec<AssociationEffect>` を実装する (Handshaking → Gated + deferred discard + Lifecycle::Gated effect)
- [x] 7.11 `Association::quarantine(&mut self, reason, now) -> Vec<AssociationEffect>` を実装する (Active/Handshaking/Gated/Idle → Quarantined + send_queue drain + deferred discard + Lifecycle::Quarantined effect。既に Quarantined 状態なら no-op)
- [x] 7.12 `Association::gate(&mut self, resume_at, now) -> Vec<AssociationEffect>` を実装する (Active → Gated + Lifecycle::Gated)
- [x] 7.13 `Association::recover(&mut self, endpoint: Option<TransportEndpoint>, now) -> Vec<AssociationEffect>` を実装する (Gated/Quarantined → Handshaking (endpoint Some) or Idle (endpoint None)。Idle/Handshaking/Active では no-op)
- [x] 7.14 `Association::enqueue(&mut self, envelope: OutboundEnvelope) -> Vec<AssociationEffect>` を実装する。状態別挙動: Active → 内部 `SendQueue` へ offer (戻り値は空 Vec)、Handshaking → deferred queue へ蓄積 (戻り値は空 Vec)、Gated → deferred queue へ蓄積 (戻り値は空 Vec)、Idle → deferred queue へ蓄積 (戻り値は空 Vec)、Quarantined → 即座に `AssociationEffect::DiscardEnvelopes { reason, envelopes: vec![envelope] }` を返す
- [x] 7.15 `Association::next_outbound(&mut self) -> Option<OutboundEnvelope>` を実装する (内部 `SendQueue` の同名メソッドに委譲)
- [x] 7.16 `Association::apply_backpressure(&mut self, signal: BackpressureSignal)` を実装する (内部 `SendQueue` の同名メソッドに委譲)
- [x] 7.17 `Association` 状態機械の unit test を作成する — 22 本 (send_queue: 4, state machine: 10, enqueue per state: 5, next_outbound/apply_backpressure: 2, Idle/Handshaking recover no-op: 1)。all `Instant::now()` absent (grep 確認済)
  - [x] Idle → Handshaking → Active 正常パス (`idle_to_handshaking_to_active_happy_path`)
  - [x] Handshaking timeout → Gated (`handshaking_timeout_transitions_to_gated_with_lifecycle`, `handshaking_timeout_with_deferred_envelopes_emits_discard`)
  - [x] Active → Quarantined (`active_to_quarantined_publishes_and_discards_pending`)
  - [x] recover(Some) で Gated → Handshaking (`recover_some_endpoint_from_gated_starts_handshake`)
  - [x] recover(Some) で Quarantined → Handshaking (`recover_some_endpoint_from_quarantined_starts_handshake`)
  - [x] recover(None) で Gated → Idle (`recover_none_from_gated_returns_to_idle`)
  - [x] Active 状態での recover は no-op (`recover_from_active_is_no_op`)
  - [x] enqueue: Active → 空 Vec + send_queue に保持 (`enqueue_in_active_pushes_into_send_queue`)
  - [x] enqueue: Handshaking → 空 Vec + deferred に保持 (`enqueue_in_handshaking_pushes_into_deferred`)
  - [x] enqueue: Gated/Idle でも deferred へ (`enqueue_in_gated_pushes_into_deferred`, `enqueue_in_idle_pushes_into_deferred`)
  - [x] enqueue: Quarantined → DiscardEnvelopes effect 返却 (`enqueue_in_quarantined_emits_discard_effect`)
  - [x] enqueue: Handshaking 中に溜めた後 handshake_accepted で SendEnvelopes effect 返却 (`deferred_envelopes_flush_on_handshake_accepted`)
  - [x] next_outbound: system 優先の取り出し順 (`next_outbound_returns_system_then_user_through_association`, `send_queue_drains_system_before_user`)
  - [x] apply_backpressure: user 側の pause/release (`apply_backpressure_propagates_to_send_queue`, `send_queue_backpressure_pauses_user_lane_but_not_system`)
- [x] 7.18 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-association-state-machine` (recover requirement 含む)。関連 Decision: 4 (state machine 図)、6 (内部可変性禁止)、7 (時刻引数化)。注力観点: 状態遷移の網羅性、`&mut self` 原則、`Instant::now()` 不呼出し、deferred queue の flush/discard タイミング、effect 出力の過不足、`enqueue`/`next_outbound`/`apply_backpressure` の Association 公開

## 8. association モジュール - SendQueue

spec: `remote-core-association-state-machine`

- [x] 8.1 `association/send_queue.rs` に `pub struct SendQueue` を定義する (system queue + user queue、`Vec<OutboundEnvelope>` ベース、`&mut self` 操作) — Section 7 の task 7.14 で SendQueue が必要なため前倒し実装
- [x] 8.2 `SendQueue::new()` および `SendQueue::with_capacity(system: usize, user: usize)` を実装する
- [x] 8.3 `SendQueue::offer(&mut self, envelope: OutboundEnvelope) -> OfferOutcome` を実装する (priority に応じて system/user に振り分け) — `OfferOutcome` は `association/offer_outcome.rs` に独立ファイルで配置。Phase A は `Accepted` のみの単一バリアント (Phase B で拡張予定)
- [x] 8.4 `SendQueue::next_outbound(&mut self) -> Option<OutboundEnvelope>` を実装する (system 優先、user は backpressure 適用時 pause)
- [x] 8.5 `SendQueue::apply_backpressure(&mut self, signal: BackpressureSignal)` を実装する
- [x] 8.6 `SendQueue` の unit test を作成する — 4 tests (`association/tests.rs` 内、`send_queue_*` prefix)
- [x] 8.7 `Association` が内部で `SendQueue` を利用することを接続する (7.14 の enqueue パスで SendQueue::offer を呼ぶ) — Active 状態の enqueue で `self.send_queue.offer(...)` を呼ぶことを確認
- [x] 8.8 【品質ゲート】Section 0 のプロトコルに従う。関連 Decision: 10 (SendQueue priority ロジック分離)。注力観点: system 優先取り出し、user 側 backpressure pause/release、`&mut self` 原則、`Association` との統合整合性

## 9. failure_detector モジュール

spec: `remote-core-failure-detector`

- [x] 9.1 `src/failure_detector.rs` および `src/failure_detector/` を作成する
- [x] 9.2 `failure_detector/heartbeat_history.rs` に `pub struct HeartbeatHistory` を定義する (`alloc::collections::VecDeque<u64>` ベースの ring buffer)
- [x] 9.3 `HeartbeatHistory::record(&mut self, interval: u64)`、`mean()`、`std_deviation()` を実装する — population std_deviation、2 サンプル未満では 0.0
- [x] 9.4 `failure_detector/phi_accrual.rs` に `pub struct PhiAccrualFailureDetector { threshold, max_sample_size, min_std_deviation, acceptable_heartbeat_pause, first_heartbeat_estimate, history, last_heartbeat_ms }` を定義する
- [x] 9.5 `PhiAccrualFailureDetector::new(...)` コンストラクタを実装する — 構築時に `first_heartbeat_estimate` 前後の 2 サンプルで history を seed (Pekko 互換)
- [x] 9.6 `PhiAccrualFailureDetector::heartbeat(&mut self, now: u64)` を実装する
- [x] 9.7 `PhiAccrualFailureDetector::phi(&self, now: u64) -> f64` を実装する — Pekko の logistic 近似 (`y * (1.5976 + 0.070566 * y * y)`) + `min_std_deviation` 適用 + NaN/Infinity クランプ。`libm::exp` / `libm::log10` 使用 (workspace dep `libm = "0.2"` を追加)
- [x] 9.8 `PhiAccrualFailureDetector::is_available(&self, now: u64) -> bool` を実装する
- [x] 9.9 `failure_detector` モジュールの unit test を作成する — 13 tests: HeartbeatHistory 5 + PhiAccrual 8 (履歴上限、phi 計算、is_available 判定、標準偏差0 でも非発散、acceptable_pause 効果)
- [x] 9.10 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-failure-detector`。関連 Decision: 7 (時刻引数化)。注力観点: `Instant::now()` 不呼出し、`min_std_deviation` による発散回避、`max_sample_size` 上限遵守、Pekko 互換のアルゴリズム動作

## 10. watcher モジュール

spec: `remote-core-watcher-state`

- [x] 10.1 `src/watcher.rs` および `src/watcher/` を作成する
- [x] 10.2 `watcher/watcher_command.rs` に `pub enum WatcherCommand { Watch { target, watcher }, Unwatch { target, watcher }, HeartbeatReceived { from, now }, HeartbeatTick { now } }` を定義する — `target`/`watcher` は `ActorPath`、`from` は `Address`、`now` は monotonic millis (`u64`)
- [x] 10.3 `watcher/watcher_effect.rs` に `pub enum WatcherEffect { SendHeartbeat { to }, NotifyTerminated { target, watchers }, NotifyQuarantined { node } }` を定義する — `to`/`node` は `Address`
- [x] 10.4 `watcher/watcher_state.rs` に `pub struct WatcherState { watching, detectors, .. }` を定義する — 追加フィールド: `targets_by_node`, `already_notified`, `detector_factory`。hashbrown + ahash::RandomState で Map<K,V> type alias を使用
- [x] 10.5 `WatcherState::new(...)` コンストラクタを実装する — `DetectorFactory` (fn pointer) を受け取り、新ノード検知時に detector を生成する
- [x] 10.6 `WatcherState::handle(&mut self, command: WatcherCommand) -> Vec<WatcherEffect>` を実装する — Watch/Unwatch/HeartbeatReceived/HeartbeatTick を dispatch
- [x] 10.7 `WatcherState` が `ActorRef`・`Sender`・`Receiver`・`async fn`・`tokio` 依存を含まないことを確認する — grep でゼロ件確認済
- [x] 10.8 `watcher` モジュールの unit test を作成する — 10 tests (Watch/Unwatch/local path 無視/同一ノード複数ターゲット/heartbeat 更新/tick 時 SendHeartbeat/長期沈黙で Terminated+Quarantined/連続 tick で duplicate なし/heartbeat で再オープン)
- [x] 10.9 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-watcher-state`。関連 Decision: 9 (状態部のみ core)。注力観点: `ActorRef`/`Sender`/`Receiver` 不保持、`async fn` 不使用、command/effect の対称性、failure detector との連携動作

## 11. instrument モジュール

spec: `remote-core-instrument`

- [x] 11.1 `src/instrument.rs` および `src/instrument/` を作成する
- [x] 11.2 `instrument/remote_instrument.rs` に `pub trait RemoteInstrument` を定義する — `on_send(&mut self, &OutboundEnvelope)` + `on_receive(&mut self, &InboundEnvelope)` フック
- [x] 11.3 `instrument/flight_recorder_event.rs` に `pub enum FlightRecorderEvent { Send, Receive, Handshake, Quarantine, Backpressure }` を定義する — 各バリアントは `authority` + `now_ms` を持ち、種別固有フィールドで拡張 (Send/Receive は correlation_id/size、Handshake は phase、Quarantine は reason、Backpressure は signal/correlation_id)
- [x] 11.4 `instrument/flight_recorder_snapshot.rs` に `pub struct RemotingFlightRecorderSnapshot` を定義する — immutable data、`events() -> &[FlightRecorderEvent]` / `len` / `is_empty` accessor
- [x] 11.5 `instrument/flight_recorder.rs` に `pub struct RemotingFlightRecorder { capacity, events: VecDeque<FlightRecorderEvent> }` を定義する
- [x] 11.6 `RemotingFlightRecorder::new(capacity: usize) -> Self` を実装する (capacity=0 で recording 無効化)
- [x] 11.7 `record_*(now: u64, ...)` 系メソッドを実装する — `record_send` / `record_receive` / `record_handshake` / `record_quarantine` / `record_backpressure`、すべて `now_ms: u64` (monotonic millis)
- [x] 11.8 `RemotingFlightRecorder::snapshot(&self) -> RemotingFlightRecorderSnapshot` を実装する
- [x] 11.9 容量超過時に最古イベントを破棄する ring buffer 動作を実装する (`pop_front` + `push_back`)
- [x] 11.10 `instrument` モジュールの unit test を作成する — 12 tests (空初期/5 種類イベント/Handshake 2 相/Backpressure Apply+Release/ring buffer eviction/capacity=0/順序保存/snapshot 不変性/RemoteInstrument trait impl)。`HandshakePhase` は独立型として `instrument/handshake_phase.rs` に配置
- [x] 11.11 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-instrument`。関連 Decision: 12 (alloc ベース ring buffer)。注力観点: transport 非依存性 (`#[cfg(feature = "tokio-transport")]` 不在)、`heapless` 非依存、容量超過時の最古破棄、時刻引数化、snapshot の immutability

## 12. provider モジュール

spec: `remote-core-actor-ref-provider`

- [x] 12.1 `src/provider.rs` および `src/provider/` を作成する
- [x] 12.2 `provider/provider_error.rs` に `pub enum ProviderError` を定義する。バリアント: `NotRemote`・`InvalidPath`・`MissingAuthority`・`UnsupportedScheme` — `Debug`/`Display`/`core::error::Error` 実装付き
- [x] 12.3 `provider/path_resolver.rs` に `pub fn resolve_remote_address(path: &ActorPath) -> Option<UniqueAddress>` を定義する (free function、struct ではない) — ActorPath の authority_endpoint + system + uid から `UniqueAddress` を構築。uid 未設定時は `0` sentinel
- [x] 12.4 `provider/remote_actor_ref.rs` に `pub struct RemoteActorRef { path: ActorPath, remote_node: RemoteNodeId }` を **data-only** 型として定義する (send/tell/ask メソッドなし)
- [x] 12.5 `RemoteActorRef` に `path()`、`remote_node()` の accessor と `Clone`・`PartialEq`・`Eq`・`Hash` のみ実装する (+ `Debug`)
- [x] 12.6 `provider/remote_actor_ref_provider.rs` に `pub trait RemoteActorRefProvider` を定義する (**remote 専用**、Decision 3-C)
- [x] 12.7 `RemoteActorRefProvider::actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError>` を宣言する — 戻り値は `RemoteActorRef`。doc comment に「local path 振り分けは adapter 責務」「local path 渡しで `NotRemote` を返す実装を推奨」「`&mut self` は caching 用途の CQS 例外」を明記
- [x] 12.8 `RemoteActorRefProvider::watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>` を宣言する — `fraktor_actor_core_rs::core::kernel::actor::Pid` を使用
- [x] 12.9 `RemoteActorRefProvider::unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>` を宣言する
- [x] 12.10 `provider/` 配下に `loopback.rs`・`tokio.rs`・`remote.rs` 等の transport 種別 provider ファイルを **作らない** ことを確認する — `ls` 確認: `path_resolver.rs`, `provider_error.rs`, `remote_actor_ref.rs`, `remote_actor_ref_provider.rs`, `tests.rs` のみ
- [x] 12.11 `RemoteActorRefProvider::actor_ref` の doc comment に「loopback 短絡は Phase B adapter の責務」を明記。`resolve_remote_address` の doc にも「authority-less path は None を返し、それが local path の signal」と明記
- [x] 12.12 `provider/` 配下に local ActorRef を構築する型 (`LocalActorRef`, `LoopbackActorRefProvider`) を **作らない** ことを確認する — grep で `LocalActorRef`/`LoopbackActorRefProvider`/`LoopbackTransport`/`PathResolver` ゼロ件
- [x] 12.13 `provider` モジュールの unit test を作成する — 10 tests (resolve: local/remote/uid、RemoteActorRef: accessors/clone/eq、stub provider: 正常解決/local 拒絶/authorityless 拒絶/watch/unwatch/local watch 拒絶)
- [x] 12.14 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-actor-ref-provider`。関連 Decision: 3-C (remote 専用 provider + adapter 責務の loopback 振り分け)。注力観点: `loopback.rs`/`tokio.rs`/`remote.rs` 不在、`RemoteActorRef` の data-only (send/tell/ask メソッド不在)、`actor_ref` が `Result<RemoteActorRef, ProviderError>` を返すこと (actor-core `ActorRef` ではない)、local path 渡しで `ProviderError::NotRemote` を返すこと、`LocalActorRef`/`LoopbackActorRefProvider` 不在 (core は local 解決責務を持たない)、`watch`/`unwatch` の宣言、`resolve_remote_address` が free function、loopback 短絡が core spec から消えて adapter 責務として明示されていること

## 13. extension モジュール (god object 解体の core 側)

spec: `remote-core-extension`

- [x] 13.1 `src/extension.rs` および `src/extension/` を作成する
- [x] 13.2 `extension/remoting_error.rs` に `pub enum RemotingError { InvalidTransition, TransportUnavailable, AlreadyRunning, NotStarted }` を定義し、`Debug`・`Display`・`core::error::Error` を実装する
- [x] 13.3 `extension/lifecycle_state.rs` に `pub struct RemotingLifecycleState` を定義する — 内部 `enum Phase { Pending, Starting, Running, ShuttingDown, Shutdown }` (Phase は private)、`&mut self` 遷移
- [x] 13.4 `RemotingLifecycleState` の状態機械を閉じた形で実装する — `transition_to_start` (Pending→Starting / StartingまたはRunningで`AlreadyRunning` / 終端状態で`InvalidTransition`)、`mark_started` (Starting→Running / それ以外で`InvalidTransition`)、`transition_to_shutdown` (Running→ShuttingDown / Pending→Shutdown ショートカット / それ以外で`InvalidTransition`)、`mark_shutdown` (ShuttingDown→Shutdown / それ以外で`InvalidTransition`)、`is_running` / `is_terminated` / `ensure_running`
- [x] 13.5 `RemotingLifecycleEvent` の **新定義は行わない** — actor-core の既存型を `use` で参照。`extension/lifecycle_event.rs` 不在 (`ls extension/` 確認済)
- [x] 13.6 `extension/remote_authority_snapshot.rs` に `pub struct RemoteAuthoritySnapshot` を定義する — `address`, `is_connected`, `is_quarantined`, `last_contact_ms: Option<u64>`, `quarantine_reason: Option<String>` の immutable data、accessor のみ (&self)
- [x] 13.7 `extension/event_publisher.rs` に `pub struct EventPublisher { system: ActorSystemWeak }` を定義する (Decision 14) — `ActorSystemWeak` が `Debug` 未実装のため `EventPublisher` の Debug は手動実装
- [x] 13.8 `EventPublisher::publish_lifecycle(&self, event: RemotingLifecycleEvent)` を実装する — `self.system.upgrade()` → `system.publish_event(&EventStreamEvent::RemotingLifecycle(event))`、drop 後は no-op
- [x] 13.9 `extension/remoting.rs` に `pub trait Remoting` を定義する — `start(&mut self)`, `shutdown(&mut self)`, `quarantine(&mut self, &Address, Option<u64>, QuarantineReason)`, `addresses(&self) -> &[Address]`
- [x] 13.10 `Remoting` trait が `transport_ref`/`bridge_factory`/`watcher_daemon`/`heartbeat_channels`/`writer`/`reader` メソッドを **持たない** — grep でゼロ件確認済
- [x] 13.11 `extension` モジュールの unit test を作成する — 14 tests: lifecycle 正常遷移 3 / 不正遷移 8 / ensure_running 2 / RemoteAuthoritySnapshot 2 (accessors/clone)
- [x] 13.12 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-extension`。関連 Decision: 5 (god object 解体表)、14 (`EventPublisher = ActorSystemWeak` 直接保持)。注力観点: `Remoting` trait が transport_ref/bridge_factory/watcher_daemon/heartbeat_channels を保持していないこと、lifecycle 状態遷移の網羅、独自 trait abstraction の不在

## 14. lib.rs 公開境界の整備

- [x] 14.1 `src/lib.rs` ですべてのサブモジュールを `pub mod` で宣言する。宣言順はアルファベット順 — `address, association, envelope, extension, failure_detector, instrument, provider, settings, transport, watcher, wire` の 11 モジュール
- [x] 14.2 `pub use` による re-export を **行わない** — `lib.rs` に `pub use` ゼロ件
- [x] 14.3 crate-level docstring に Pekko Artery との対応関係、no_std / no async / `&mut self` 原則、時刻引数化方針を明記する — **Pekko Artery correspondence 表を追加** (11 モジュール × Pekko コンポーネントのマッピング)、no_std/no async/&mut self 原則、monotonic millis の契約、公開境界 (`pub use` 不使用の方針) を明記
- [x] 14.4 【品質ゲート】Section 0 のプロトコルに従う。参照: `module-wiring` dylint ルール、既存 `modules/remote/src/lib.rs` の `// pub use 禁止` コメント。注力観点: `pub mod` アルファベット順、`pub use` 再エクスポートの不在、crate-level docstring の内容完全性 (Pekko Artery 対応・no_std/no async/&mut self 原則・時刻引数化方針)

## 15. CI / lint 検証

- [x] 15.1 `cargo build -p fraktor-remote-core-rs` が警告ゼロで成功することを確認する — 警告ゼロで Finished
- [x] 15.2 `cargo build -p fraktor-remote-core-rs --no-default-features` (no_std build) が成功することを確認する — 成功
- [x] 15.3 `cargo test -p fraktor-remote-core-rs` がすべてのテストで成功することを確認する — **125 tests 全 passed**
- [x] 15.4 既存 dylint (mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix) が新クレートに対してもエラーなしでパスすることを確認する — **8 dylint 全 passed**
- [x] 15.5 `modules/remote-core/src/` 配下を grep し、`tokio`・`async_std`・`async-std`・`tokio_util`・`async_trait`・`futures` のヒットがゼロであることを確認する — 実コードゼロ件 (doc comment 5 件のみ: lib.rs docstring と phi_accrual.rs の「adapter 側で使うべき例」の記述)
- [x] 15.6 `modules/remote-core/src/` 配下を grep し、`#[cfg(not(test))]` スコープで `use std::` のヒットがゼロであることを確認する — 実コードゼロ件 (doc comment 1 件のみ: lib.rs の設計契約記述)
- [x] 15.7 `modules/remote-core/src/` 配下を grep し、`#[cfg(feature = "tokio-transport")]` 等の transport 実装ゲートが0件であることを確認する — ゼロ件
- [x] 15.8 `modules/remote-core/src/` 配下の公開 API シグネチャを grep し、戻り値に `Guard`・`MutexGuard`・`RwLockReadGuard`・`SpinSyncMutexGuard` を含むメソッドが存在しないことを確認する — ゼロ件
- [x] 15.9 `cargo doc -p fraktor-remote-core-rs --no-deps` が警告ゼロで成功することを確認する — 警告ゼロで Finished

## 16. Phase A 完了チェック

- [x] 16.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認する — EXIT=0、全モジュールの全テスト passing。clippy の修正: module_inception 回避のため `address/address.rs` → `address/base.rs`, `association/association.rs` → `association/base.rs` にリネーム。その他 15 個の const-fn 追加、2 個の into_parts must_use 追加、watcher_state on_unwatch を `&ActorPath` 参照渡しに変更、Codec trait に `# Errors` セクション追加、test の `matches!(..., Some(_))` を `.any()` に書き換え、`unreachable!()` を `filter_map` に書き換え、authority_string を `remote.to_string()` へ簡素化
- [x] 16.2 `openspec validate remote-redesign` を実行し、proposal/design/specs/tasks の整合性が取れていることを確認する — "Change 'remote-redesign' is valid"
- [x] 16.3 Phase A 完了を確認する。**本時点では change の archive を行わない** (Decision 15 により、archive は Phase E 完了時に一度だけ実施)

---

# Phase B: remote-adaptor-std クレートの新設と実装

Phase B は `fraktor-remote-adaptor-std-rs` クレートを新設し、Phase A で定義した core port を std + tokio で実装する。完了時点で新クレート2つ (core + adaptor-std) が揃うが、依然として旧 `modules/remote/` は残っており、依存元は旧クレートを使い続ける。Phase B 単体でも archive しない。

## 17. remote-adaptor-std クレート骨格

- [ ] 17.1 `modules/remote-adaptor-std/` ディレクトリを新規作成する
- [ ] 17.2 `modules/remote-adaptor-std/Cargo.toml` を作成する (`name = "fraktor-remote-adaptor-std-rs"`、`edition = "2024"`、ライセンス・description 等を他の `*-adaptor-std` と統一)
- [ ] 17.3 `[dependencies]` に `fraktor-remote-core-rs`、`fraktor-actor-core-rs`、`fraktor-actor-adaptor-rs`、`fraktor-utils-rs`、`tokio` (rt-multi-thread, net, sync, time, io-util)、`tokio-util` (codec)、`bytes`、`tracing` 等を追加する
- [ ] 17.4 `[lints] workspace = true` を設定する
- [ ] 17.5 ワークスペースルートの `Cargo.toml` の `members` に `modules/remote-adaptor-std` を追加する
- [ ] 17.6 `modules/remote-adaptor-std/src/lib.rs` を作成する (既存 `modules/cluster-adaptor-std/src/lib.rs` のパターン踏襲)
- [ ] 17.7 `cargo build -p fraktor-remote-adaptor-std-rs` が成功することを確認する
- [ ] 17.8 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-adaptor-std-package`。注力観点: Cargo.toml メタデータ・依存の spelling (core/adapter 両方)・lib.rs 属性・ワークスペース登録

## 18. tcp_transport モジュール (Pekko Artery TCP 相当)

- [ ] 18.1 `src/tcp_transport.rs` および `src/tcp_transport/` を作成する
- [ ] 18.2 `tcp_transport/frame_codec.rs` に `tokio_util::codec::{Encoder, Decoder}` を実装する (core の `Codec<T>` を呼び出して tokio の Framed 統合層を作る)
- [ ] 18.3 `tcp_transport/server.rs` に `pub struct TcpServer` を定義し、`TcpListener::bind` → accept loop → inbound task spawn のロジックを実装する
- [ ] 18.4 `tcp_transport/client.rs` に `pub struct TcpClient` を定義し、`TcpStream::connect` → handshake → outbound/inbound task spawn を実装する
- [ ] 18.5 `tcp_transport/tcp_transport.rs` に `pub struct TcpRemoteTransport` を定義し、core の `RemoteTransport` trait を実装する
- [ ] 18.6 `TcpRemoteTransport::start` / `shutdown` / `send(envelope)` / `quarantine` / `addresses` / `default_address` / `local_address_for_remote` を実装する
- [ ] 18.7 unit test と統合テスト (2ノード間の echo) を作成する
- [ ] 18.8 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-adaptor-std-tcp-transport`。注力観点: `RemoteTransport` trait 実装の完全性・Framed codec 統合・bind/connect のエラーハンドリング・`Instant::now()` が adapter 側でのみ呼ばれていること

## 19. association_runtime モジュール (Association を駆動する tokio task 群)

- [ ] 19.1 `src/association_runtime.rs` および `src/association_runtime/` を作成する
- [ ] 19.2 `association_runtime/association_shared.rs` に `pub struct AssociationShared = ArcShared<SpinSyncMutex<Association>>` または同等の AShared パターン型を定義する (Phase A で延期された共有ラッパー)
- [ ] 19.3 `association_runtime/association_registry.rs` に `pub struct AssociationRegistry { entries: BTreeMap<UniqueAddress, AssociationShared> }` を定義し、per-remote の Association を管理する
- [ ] 19.4 `association_runtime/outbound_loop.rs` に Association の送信ループ tokio task を実装する (`next_outbound` を呼び、取り出した envelope を `TcpRemoteTransport::send` に渡す)
- [ ] 19.5 `association_runtime/inbound_dispatch.rs` に受信ループ tokio task を実装する (TCP から受信した frame を core の `Association::handshake_accepted` や `enqueue` 相当に渡し、effect 列を実行)
- [ ] 19.6 `association_runtime/handshake_driver.rs` に handshake タイムアウト driver を実装する (`tokio::time::sleep` で経過を計測し、`Association::handshake_timed_out(now_ms: u64 /* monotonic millis */)` を呼ぶ。`Instant::now()` の差分を millis に変換して渡す)
- [ ] 19.7 `association_runtime/system_message_delivery.rs` に ack-based redelivery 実装を追加する (core の `AckPdu` と組み合わせ、sequence number 管理とリトライロジック)
- [ ] 19.8 Phase A で延期された `RemoteSettings` の ack 関連フィールドを本 Phase で追加する: `RemoteSettings` に `ack_send_window`・`ack_receive_window` を追加し、**同一 change 内の既存 capability `remote-core-settings` を更新する**。新 capability `remote-core-settings-ack` は作成しない
- [ ] 19.9 unit test と統合テスト (handshake + ack + redelivery) を作成する
- [ ] 19.10 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-adaptor-std-runtime`。注力観点: Association state machine との整合性・handshake timeout driver が monotonic millis を渡していること・ack-based redelivery の正しさ・`AssociationShared` の同期境界

## 20. watcher_actor モジュール (WatcherState を actor 化する層)

- [ ] 20.1 `src/watcher_actor.rs` および `src/watcher_actor/` を作成する
- [ ] 20.2 `watcher_actor/watcher_actor.rs` に core の `WatcherState` を保持する actor を実装する (fraktor-actor-core-rs の actor API を使用)
- [ ] 20.3 tokio timer で heartbeat tick を発生させ、`WatcherState::handle(WatcherCommand::HeartbeatTick { now: u64 /* monotonic millis */ })` を呼ぶ
- [ ] 20.4 effect 列に `SendHeartbeat { to }` が含まれる場合は `TcpRemoteTransport::send` または control channel 経由で heartbeat を送信する
- [ ] 20.5 unit test を作成する (heartbeat tick のトリガー、fake failure detector と組み合わせた生存判定)
- [ ] 20.6 【品質ゲート】Section 0 のプロトコルに従う。注力観点: `WatcherState` が actor 内で `&mut self` アクセスされていること・timer 駆動の時刻が monotonic であること

## 21. provider-dispatch モジュール (loopback 振り分け)

- [ ] 21.1 `src/provider.rs` および `src/provider/` を作成する
- [ ] 21.2 `provider/dispatch.rs` に `pub struct StdRemoteActorRefProvider` を定義し、以下のメンバを持つ:
  - `local_address: UniqueAddress` — ローカルの `UniqueAddress`
  - `local_provider: ActorRefProviderShared<LocalActorRefProvider>` または同等の actor-core 公開型 — actor-core の local actor ref provider 参照
  - `remote_provider: Box<dyn RemoteActorRefProvider>` — core の remote provider 参照
- [ ] 21.3 `provider/provider_dispatch_error.rs` に `pub enum StdRemoteActorRefProviderError` を定義する。少なくとも `NotRemote`、`CoreProvider(ProviderError)`、`LocalProvider(ActorError)`、`RemoteSenderBuildFailed` 相当のバリアントを持たせ、adapter 側の分岐・委譲・wiring 失敗を表現できるようにする
- [ ] 21.4 `StdRemoteActorRefProvider::actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, StdRemoteActorRefProviderError>` を実装する。分岐規則は 3 つ: (1) `authority_endpoint().is_none()` の通常 local path はそのまま `local_provider.actor_ref(path)` に委譲、(2) authority ありで `resolve_remote_address(&path)` の結果の **Address 部分** がローカル `self.local_address.address` と一致し、かつ path 側 uid が `0` または `self.local_address.uid` と一致する場合は **authority を持たない local 等価 path に正規化してから** `local_provider.actor_ref(local_equivalent_path)` に委譲、(3) authority ありで上記条件を満たさない場合は `remote_provider.actor_ref(path)` を呼んで `RemoteActorRef` を取得し、remote sender (内部で `TcpRemoteTransport` を呼ぶ) を構築して actor-core の `ActorRef` にラップする
- [ ] 21.5 `StdRemoteActorRefProvider::watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), StdRemoteActorRefProviderError>` および `unwatch` を実装する。**remote path 専用** とし、remote path では `remote_provider.watch/unwatch` に委譲、local path では `Err(StdRemoteActorRefProviderError::NotRemote)` を返す。local death watch は local `ActorRef` 解決後に actor-core の通常経路 (`ActorContext::watch` 等) で扱い、この型は local watch の委譲窓口にはしない
- [ ] 21.6 この「adapter 側の loopback 振り分け」が core spec `remote-core-actor-ref-provider` の「loopback 短絡の実装責務は adapter にある」要件 (Decision 3-C) を満たすことを確認する。あわせて `watch/unwatch` が remote 専用 helper であり、local watch は actor-core の通常経路で扱うこと、および adapter 側は core `ProviderError` を `StdRemoteActorRefProviderError::CoreProvider(...)` として保持することを doc comment に明記する。さらに **authority なし local path も local 分岐へ入る** ことを明記する
- [ ] 21.7 unit test を作成する (`authority_endpoint().is_none()` の通常 local path がそのまま local_provider に渡されること、ローカル authority path が authority なし local 等価 path に正規化されてから local_provider に渡されること、**path 側 uid=0 でも Address 一致なら local 分岐に入ること**、**path 側 uid と local uid が両方 non-zero で不一致なら remote 分岐に入ること**、remote path で remote_provider が呼ばれること、remote path の watch/unwatch が remote_provider に委譲されること、local path の watch/unwatch が `StdRemoteActorRefProviderError::NotRemote` を返すこと、local provider の `ActorError` が `StdRemoteActorRefProviderError::LocalProvider(...)` に変換されること、mock provider を使った振り分けの検証)
- [ ] 21.8 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-adaptor-std-provider-dispatch`。関連 Decision: 3-C。注力観点: loopback 振り分けの正しさ、**authority なし local path も local 分岐へ入ること**、**uid=0 が wildcard として扱われること**、core `RemoteActorRefProvider` が local path を受け取らないこと、`watch/unwatch` が remote 専用で local path を拒否すること、adapter 側エラー境界 (`StdRemoteActorRefProviderError`) が core `ProviderError` と actor-core `ActorError` を適切にラップしていること、actor-core の local provider との統合

## 22. extension installer モジュール

- [ ] 22.1 `src/extension_installer.rs` を作成する
- [ ] 22.2 `StdRemoting` 型を定義し、core の `Remoting` trait を実装する (`start`/`shutdown`/`quarantine`/`addresses`)
- [ ] 22.3 `StdRemoting` 内部で `TcpRemoteTransport`・`AssociationRegistry`・`WatcherActor`・`StdRemoteActorRefProvider` を保持し、起動時に配線する
- [ ] 22.4 actor system extension として登録する `RemotingExtensionInstaller` を実装する
- [ ] 22.5 既存 `modules/remote/src/std/remoting_extension_installer.rs` を参照しつつ移植する (旧ファイルは残したまま、新実装を新クレート側に作る)
- [ ] 22.6 unit test を作成する (extension 登録、lifecycle 状態遷移)
- [ ] 22.7 【品質ゲート】Section 0 のプロトコルに従う。注力観点: god object 分解結果の各責務が StdRemoting 内で正しく配線されていること、core の `Remoting` trait が runtime 配線メソッドを持っていないこと (Phase A 側の要件)

## 23. Phase B 完了チェック

- [ ] 23.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないこと (新クレート2つ追加後の全 build/test 通過)
- [ ] 23.2 新クレート `fraktor-remote-adaptor-std-rs` 単独の `cargo test -p fraktor-remote-adaptor-std-rs` が通ること
- [ ] 23.3 **本時点では change の archive を行わない**

---

# Phase C: 統合テストの移植

Phase C は既存 `modules/remote/tests/` および関連統合テストを新クレート群に移植する。単体テストは Phase A/B で作成済みのため、本 Phase は統合レベル (actor system 起動 + 2ノード間通信 + handshake + watch + failure detection) のテストを対象とする。

## 24. 統合テストの移植

- [ ] 24.1 `modules/remote/tests/` の各統合テストを列挙し、移植先 (新 core か新 adaptor-std か) を決定する
- [ ] 24.2 core 側の統合テストを `modules/remote-core/tests/` に移植する (no_std で動くもの、主に state machine レベル)
- [ ] 24.3 adapter 側の統合テストを `modules/remote-adaptor-std/tests/` に移植する (tokio runtime が必要なもの、主に 2ノード間通信)
- [ ] 24.4 `modules/remote/tests/quickstart.rs` を新クレート向けに書き直す
- [ ] 24.5 移植後の旧テスト (`modules/remote/tests/`) は Phase E の削除対象として残す (Phase C では削除しない)
- [ ] 24.6 `cargo test -p fraktor-remote-core-rs -p fraktor-remote-adaptor-std-rs` が全通過することを確認する
- [ ] 24.7 【品質ゲート】Section 0 のプロトコルに従う。注力観点: 既存テストの移植漏れなし・新旧両方で同じテストが通ること・統合テストが実運用シナリオをカバーしていること

## 25. Phase C 完了チェック

- [ ] 25.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないこと
- [ ] 25.2 **本時点では change の archive を行わない**

---

# Phase D: 依存元の切り替え

Phase D は現在 `fraktor-remote-rs` に依存しているモジュール (主に `modules/cluster-adaptor-std/`) を新クレート群への依存に切り替える。この Phase 完了時点で新クレートが実運用経路で検証される。

## 26. 依存元モジュールの特定と計画

- [ ] 26.1 `grep -rn 'fraktor-remote-rs' modules/*/Cargo.toml` で依存元モジュールを列挙する
- [ ] 26.2 各モジュールの依存を `fraktor-remote-core-rs` + `fraktor-remote-adaptor-std-rs` に置き換える計画を立てる (使用している API をマッピング)
- [ ] 26.3 計画を文書化する (本 tasks.md の注記または別ファイル)

## 27. 依存切り替え実施

- [ ] 27.1 `modules/cluster-adaptor-std/Cargo.toml` の `fraktor-remote-rs` 依存を `fraktor-remote-core-rs` + `fraktor-remote-adaptor-std-rs` に置き換える
- [ ] 27.2 `modules/cluster-adaptor-std/src/` 配下で `fraktor_remote_rs::...` import を新クレートの path に修正する
- [ ] 27.3 `cargo build -p fraktor-cluster-adaptor-rs` が成功することを確認する
- [ ] 27.4 `cargo test -p fraktor-cluster-adaptor-rs` が全通過することを確認する
- [ ] 27.5 他の依存元モジュールがあれば同様に切り替える
- [ ] 27.6 この時点で旧 `modules/remote/` は **誰からも参照されていない** 状態になる (verify: `grep -rn 'fraktor-remote-rs' modules/*/Cargo.toml` の結果が空)
- [ ] 27.7 `./scripts/ci-check.sh ai all` を実行し、全通過することを確認する (新クレート経路での検証)
- [ ] 27.8 【品質ゲート】Section 0 のプロトコルに従う。注力観点: 依存切り替え漏れゼロ・cluster-adaptor-std 等の動作が切り替え前後で等価であること・新クレート経路での CI 全通過

## 28. Phase D 完了チェック

- [ ] 28.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないこと
- [ ] 28.2 新クレート経路でのリグレッションなし
- [ ] 28.3 **本時点では change の archive を行わない** (次の Phase E で旧削除後に archive)

---

# Phase E: 旧実装の削除と archive

Phase E は旧 `modules/remote/` ディレクトリを完全削除し、本 change を archive する。この Phase 完了時点で legacy-code-temporary-usage.md ルール3 に構造的準拠する (archive 時点で旧実装が残っていない)。

## 29. 旧 modules/remote/ の削除

- [ ] 29.1 `modules/remote/` 配下のすべてのファイル・ディレクトリを削除する
- [ ] 29.2 ワークスペース `Cargo.toml` の `members` から `modules/remote` エントリを削除する
- [ ] 29.3 `cargo build --workspace` が成功することを確認する
- [ ] 29.4 `cargo test --workspace` が全通過することを確認する
- [ ] 29.5 `grep -rn 'fraktor-remote-rs' .` の結果がゼロ件であることを確認する (他の場所で残参照がないこと。ドキュメント類の言及は除外)
- [ ] 29.6 `grep -rn 'modules/remote/' .` の結果がゼロ件であることを確認する (openspec 配下とドキュメント類を除外)
- [ ] 29.7 【品質ゲート】Section 0 のプロトコルに従う。対象 spec: `remote-core-package` (legacy removal 要件)。注力観点: 削除の完全性・残参照ゼロ・ワークスペース member 整合性

## 30. 最終 CI と change の archive

- [ ] 30.1 `./scripts/ci-check.sh ai all` を実行し、**完全無エラー** であることを確認する
- [ ] 30.2 `cargo doc --workspace --no-deps` が警告ゼロで成功することを確認する
- [ ] 30.3 `openspec validate remote-redesign` を実行し、proposal/design/specs/tasks の整合性が取れていることを確認する
- [ ] 30.4 `openspec archive remote-redesign` を実行し、本 change の全 spec を `openspec/specs/` に確定する。この archive が legacy-code-temporary-usage.md ルール3 への構造的準拠点である (旧実装は既に存在しない状態)
- [ ] 30.5 archive 後、`openspec/changes/remote-redesign/` が存在しないことを確認する (archive は change dir を `openspec/changes/archive/YYYY-MM-DD-remote-redesign/` へ移動する前提)
