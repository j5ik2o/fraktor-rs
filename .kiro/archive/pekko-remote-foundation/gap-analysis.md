# ギャップ分析: pekko-remote-foundation

## 1. 現状調査
### 1.1 既存資産とレイアウト
- **アドレッシング/設定**: `modules/actor/src/core/actor_prim/actor_path/*` と `ActorPathFormatter` が Pekko URI 互換の canonical path を生成し、`ActorSystemConfig` + `RemotingConfig` (`modules/actor/src/core/config/*.rs`) が canonical host/port・隔離期間を設定する。
- **システム状態とイベント**: `SystemStateGeneric` (`modules/actor/src/core/system/system_state.rs`) が `RemoteAuthorityManager`、EventStream、DeadLetter を保持し、remoting 設定を `apply_actor_system_config` でパス ID に反映する。
- **Authority 管理**: `RemoteAuthorityManagerGeneric` (`modules/actor/src/core/system/remote_authority.rs`) と `AuthorityState` が Unresolved/Connected/Quarantine を管理し、`ActorSelectionResolver` が送信前チェックと defer を実施する。
- **観測フック**: EventStream (`modules/actor/src/core/event_stream/event_stream_event.rs`) に `RemoteAuthority` イベント種別があり、SystemState が state 変化時に発火。（`modules/actor/src/core/system/system_state.rs:794-818`）
- **シリアライゼーション基盤**: `modules/actor/src/core/serialization/*` に Pekko 互換 `SerializedMessage`、serializer registry、manifest binding が揃っている。
- **テスト資産**: `modules/actor/tests/actor_path_e2e.rs` が RemotingConfig 統合・Authority 状態遷移を e2e で検証済み。
- **新規クレート**: `modules/remote` は空の `lib.rs` のみで、実装は未着手。

### 1.2 パターンと制約
- 2018 モジュール/1ファイル1型ルール (`.kiro/steering/structure.md`)。`cfg(feature = "std")` を core で使えないため、no_std 前提の抽象（`RuntimeToolbox`）を境界にする必要がある。
- イベント可観測性は EventStream に集約。RemoteLifecycle 相当もここへ集めるのが自然。
- `RemoteAuthorityManager` は state だけを担っており、I/O や handshake は未実装。Endpoint/Transport 層を別クレートで定義する余地がある。

### 1.3 インテグレーション面
- ActorSystem からは `state.remote_authority_manager()` にしか触れられず、Transport/Endpoint が存在しないためメッセージは常にローカル配送。
- Serialization レイヤは既に Pekko 形式を想定しているので、Remoting からは `SerializedMessage` と manifest を利用可能。
- EventStream は `RemoteAuthorityEvent` のみで、Remoting/Transport のライフサイクル（Listen/Stopped/Error）をまだ表現していない。

## 2. 要件別ギャップマップ
| 要件領域 | 関連資産 | ギャップ/制約 | Research Needed |
| --- | --- | --- | --- |
| Remoting 構成 (Req1) | `RemotingConfig`, `ActorSystemConfig::with_remoting`, `SystemState::apply_actor_system_config` | 実際の `Remoting`/`RemoteTransport` 実装や Transport 選択ロジックが存在せず、バックプレッシャーフック/フレーミング実装も未着手。std/no_std で差し替える抽象インターフェースが必要。 | Pekko `RemoteTransport`/`Remoting.scala` のライフサイクル API を Rust 向けにどうマッピングするか。Tokio/embassy で共有できる Transport trait 設計。 |
| EndpointManager と隔離 (Req2) | `RemoteAuthorityManager`, `SystemState::remote_authority_*`, `ActorSelectionResolver` | Authority 状態しかなく、接続確立/handshake/UID 検証/遅延キュー flush を担う Endpoint actor・Registry が不在。Quarantine のタイムアウトは設定できるが、Endpoint ごとの writer/reader が無いため実際のメッセージ再送や gating ができない。 | Pekko `EndpointManager`/`EndpointRegistry` の state machine 解析、Rust 版で actor-less FSM をどう構築するか。UID/Address 管理のフォーマット。 |
| EndpointWriter 優先制御 (Req3) | `AnyMessageGeneric`, `SystemMessage`, `SerializedMessage`, DeadLetter 管理, EventStream | リモート宛の queue や writer/reader、system message 優先送出の経路が無い。`reply_to` を payload に入れるルールはあるが、Remote Path 付与/manifest 埋め込み処理が未定義。`modules/remote` に Transport ごとの writer 実装を新設する必要。 | Pekko `EndpointWriter/EndpointReader` のメッセージフロー、priority mailbox、AckedDelivery の復元方法。Rust の mailbox へどう統合するか。 |
| EventPublisher/観測 (Req4) | EventStream(`RemoteAuthority` event), DeadLetter/Log, `SystemState::poll_remote_authorities` | RemotingLifecycle (Listen/Shutdown/Error) イベント、FailureDetector からの Suspect/Reachable 通知、メトリクス集約 (`RemotingFlightRecorder`) が無い。Remote 健全性 API (snapshot) も未実装。 | Pekko `EventPublisher`, `RemotingFlightRecorder`, `FailureDetector` (PhiAccrual) の Rust 変換。Tokio/embassy 環境でのハートビート送出方法。 |
| RemoteActorRefProvider | なし | 現行 ActorSystem はローカル構築のみで、RemoteDaemon/RemoteActorRef を生成する仕組みが無い。 | Pekko `RemoteActorRefProvider` の責務・API を Rust ActorSystem にどう溶け込ませるか。 |

**共通の制約**: `modules/remote` クレートが空であるため、Transport/Endpoint/Fault detection/Monitoring を丸ごと実装する必要がある。std/no_std 両対応、`RuntimeToolbox` 経由で同期原語を抽象化する必要がある。

## 3. 実装アプローチ候補
### Option A: 既存 actor クレートへ直接追加
- **内容**: `modules/actor` 内に Remoting/Endpoint/Transport 実装を追加し、SystemState から直接呼び出す。
- **利点**: 既存の `ArcShared`, `RuntimeToolbox`, EventStream をそのまま利用。API 変更が少なく済む。
- **欠点**: core クレートが肥大化し、no_std 境界に std/Tokio 依存が混入するリスク (`cfg-std-forbid`)。Transport 実装単位の差し替えが困難。

### Option B: 新規 `modules/remote` に骨格を構築
- **内容**: 今回追加した `modules/remote` を本命として、`RemoteTransport` トレイト、`Remoting`（Facade）、`EndpointManager/Writer/Reader`, `FailureDetector`, `RemotingFlightRecorder` を個別ファイルで実装。`modules/actor` からは REM API を介して利用。
- **利点**: 責務分離が明瞭で、std/no_std 切替・Transport plugin (Tokio vs bare metal) をクレート境界で管理できる。型/trait により interface を固定化しやすい。
- **欠点**: ActorSystem と Remote クレート間の API 設計・所有権/同期の整理に追加コスト。短期的には boilerplate が増える。

### Option C: ハイブリッド (State は actor、Transport は remote クレート)
- **内容**: Authority/Registry など stateful なロジックは `modules/actor` に残し、Transport/EndpointWriter/FailureDetector など I/O 層を `modules/remote` に切り出す。
- **利点**: 既存 state を再利用しやすく、段階導入が可能。`RemoteAuthorityManager` を維持しつつ Transport を差し込める。
- **欠点**: クレート間でイベントや state を跨るため、API が肥大化しやすい。責務が分散し過ぎると保守性が落ちる。

## 4. 努力・リスク評価
- **Effort**: **L (1〜2週間)** — Endpoint/Transport/Fault detection/Monitoring をフルで追加する必要があり、Tokio/no_std の2系統 Transport、EventStream/Serializer 統合、CI 対応など多岐にわたる。
- **Risk**: **High** — ネットワーク I/O と actor ランタイムの境界を新設するため、デッドロックやメモリ使用、API 互換性に不確定要素が多い。Pekko との互換性も確認が必要で、テスト行程が複雑。

## 5. デザインフェーズへの持ち越し事項
- Transport トレイト設計（Tokio TCP / embassy / テストダブル）とバッファリング戦略。
- Handshake/UID/Quarantine 仕様の Rust 版 FSM。`AddressUidExtension` と同等の仕組みが必要。
- FailureDetector (PhiAccrual) の no_std 実装可否、ハートビートスケジューリング。RemoteWatcher の actor 化 or state machine 化。
- RemotingFlightRecorder の最小実装（NoOp + feature-based recorder）と EventStream へのメトリクス露出。
- RemoteActorRefProvider 互換の API を ActorSystem にどう統合するか（新しい builder, extension?）。

---
この分析は `.kiro/settings/rules/gap-analysis.md` に従い、要求とコードベースの差分を整理したものです。次はこの結果を踏まえて `/prompts:kiro-spec-design pekko-remote-foundation` で設計フェーズへ進むことを推奨します。
