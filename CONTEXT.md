# fraktor-rs Actor System Context

fraktor-rs は、actor / remote / cluster / stream / persistence の各ドメインに対して移植可能な実行契約を定義する、Rust 製のアクターシステムである。

## Language

### Actor Execution

**Actor Cell (アクターセル)**:
Actor (アクター) インスタンスの mailbox ディスパッチ、ライフサイクル、supervision、子関係、DeathWatch (死亡監視)、コンテキストの副作用を調停する actor-core-kernel の実行コンテナ。Actor (アクター) の user behavior や ActorRef (アクター参照) の address identity とは別の実行境界である。
_Avoid_: Runtime, Actor, ActorRef, Mailbox Owner

**Actor Cell Facet (アクターセルファセット)**:
Actor Cell (アクターセル) の同一型実装を dispatch、lifecycle、fault handling、children、DeathWatch (死亡監視)、Receive Timeout (受信タイムアウト) などの責務単位で追跡するための内部実装境界。public trait や利用者向け API を増やすための境界ではない。
_Avoid_: Runtime, Public Facet API, Trait Facet, ActorCell Subclass

**Actor System State (アクターシステム状態)**:
actor system のスコープを持つ状態を、既存の accessor 経由で扱うファサード。実行補助、dispatcher / mailbox、event / logging、guardian / cells、remote / provider、scheduler / lifecycle などの System State Registry (システム状態レジストリ) を束ねるが、それぞれの subsystem behavior を直接所有する概念ではない。
_Avoid_: Runtime, Global State, God Object, Shared State Bag

**System State Registry (システム状態レジストリ)**:
Actor System State (アクターシステム状態) の内側で、dispatcher / mailbox、event / logging、guardian / cells など単一の subsystem の状態所有を担う private registry。外部 crate に公開する registry handle ではなく、既存のファサードから委譲される内部境界である。
_Avoid_: Runtime, Public Registry Handle, Shared Global Registry, Service Locator

**DeathWatch (死亡監視)**:
ある actor が別の actor の終了を観測し、終了通知を受け取る、actor 実行上の観測契約。Lifecycle Event (ライフサイクルイベント) の発行や child registry の所有とは区別する。
_Avoid_: Runtime, Lifecycle Event, Child Registry, Failure Detection

**Receive Timeout (受信タイムアウト)**:
actor が一定期間、timeout に影響する user message を受信しなかったときに timeout シグナルを届ける実行契約。scheduler の停止や mailbox の idle 状態そのものではなく、actor behavior に見える inactivity シグナルである。
_Avoid_: Runtime, Scheduler Timeout, Mailbox Idle Timeout, Shutdown Timeout

**EventStream (イベントストリーム)**:
actor system 内で dead letter、unhandled message、logging event などを購読者へ公開する、具体的な event publication surface。汎用的な EventBus (イベントバス) の分類契約や Cluster PubSub の topic delivery とは別の、actor-local な event surface である。
_Avoid_: Logging Backend, Cluster PubSub, Topic Delivery

**EventBus (イベントバス)**:
event を classifier と subscriber registry に基づいて配信する、actor-core-kernel の汎用的な event distribution abstraction。EventStream (イベントストリーム) はこの概念を使う concrete surface であり、Cluster PubSub や logging backend ではない。
_Avoid_: Cluster PubSub, Logging Backend, Message Queue

**Message Observability (メッセージ観測性)**:
actor の message flow や state transition を、EventStream (イベントストリーム)、remote boundary、debugging から観測できるようにする actor 実行上の契約。logging backend や tracing 出力先の選択ではなく、観測可能な protocol / marker / event の語彙を所有する。
_Avoid_: Logging Backend, Tracing Output, Serialization Format

**EventBus Classification (イベントバス分類)**:
EventBus (イベントバス) が event を subscriber へ配信するための classification strategy と subscriber registry の契約。PubSub Mediator Protocol (PubSubメディエータプロトコル) や logging backend ではなく、actor-core-kernel 内の event distribution semantics を表す。
_Avoid_: Cluster PubSub, Logging Backend, Topic Registry

**Mailbox Resolution (メールボックス解決)**:
actor の requirement や、dispatcher / deploy / props / default source から、actor に使う mailbox queue contract を決める、実行前の選択契約。mailbox run loop、enqueue blocking behavior、queue implementation そのものとは区別する。
_Avoid_: Mailbox Runtime, Queue Implementation, Blocking Wait

**Coordinated Shutdown (協調シャットダウン)**:
actor system の shutdown phase に沿って task を登録・解除・実行し、actor termination を順序付きに待てる lifecycle contract。OS signal handling や process exit、full cluster shutdown command ではない。
_Avoid_: Process Exit, OS Signal Handling, Full Cluster Shutdown Command

**Typed Actor API Boundary (型付きアクターAPI境界)**:
untyped actor kernel の上に、message type、typed behavior、typed system setup、typed extension surface を載せる利用者向け actor API 境界。kernel の Actor Cell (アクターセル) や Actor System State (アクターシステム状態) そのものではない。
_Avoid_: Kernel API, Actor Cell, Behavior Implementation

**Typed Receptionist (型付きレセプショニスト)**:
typed actor が service key に対する登録、解除、購読、検索を行うための discovery surface。Cluster Discovery や Cluster PubSub ではなく、Typed Actor API Boundary (型付きアクターAPI境界) 上の actor-local service discovery 契約である。
_Avoid_: Cluster Discovery, Cluster PubSub, Global Service Registry

**Receptionist Setup (レセプショニスト設定)**:
Typed Receptionist (型付きレセプショニスト) の facade / extension factory を typed ActorSystem setup に差し込むための構築時契約。clustered receptionist の実装や wire protocol ではなく、既存 behavior と代替 facade の install boundary を表す。
_Avoid_: Clustered Receptionist Execution, Receptionist Wire Protocol, Behavior Implementation

**Dead Letter (デッドレター)**:
配送できなかった actor message を EventStream (イベントストリーム) で観測可能にする message observability event。delivery retry、mailbox overflow strategy、logging backend そのものではない。
_Avoid_: Retry Queue, Mailbox Overflow Strategy, Logging Backend

**Dead Letter Suppression (デッドレター抑制)**:
message が Dead Letter (デッドレター) として公開されるときに、通常の dead letter とは別の suppressed observation として扱うための marker contract。message delivery を成功扱いにしたり、delivery failure を隠したりするものではない。
_Avoid_: Delivery Success, Silent Drop, Logging Filter

**Actor Selection (アクター選択)**:
actor path を解決して ActorRef (アクター参照) に到達するための selection contract。ActorRef (アクター参照) そのものや remote transport lookup ではなく、path resolution と ask composition の境界である。
_Avoid_: ActorRef, Remote Transport Lookup, Cluster Discovery

**Circuit Breaker (サーキットブレーカー)**:
失敗の蓄積に応じて Closed / Open / HalfOpen の状態を遷移させ、外部呼び出しの遮断と再試行タイミングを制御する actor pattern contract。supervision policy や scheduler そのものではない。
_Avoid_: Supervision Policy, Scheduler Timer, Retry Loop

**Bounded Mailbox Compatibility (有界メールボックス互換性)**:
bounded mailbox の容量超過時に reject / dead letter / timeout wait の互換挙動をどう扱うかの mailbox contract。Mailbox Resolution (メールボックス解決) や queue implementation そのものではなく、async-first default と compatibility option の境界である。
_Avoid_: Mailbox Resolution, Queue Implementation, Blocking Executor

**Kernel Public Surface (カーネル公開面)**:
actor kernel が外部 crate へ公開する利用者向け contract の集合。internal cell / system implementation や public re-export された低レベル型の一覧ではなく、公開すべき境界を判断するための語彙である。
_Avoid_: Internal Surface, Public Re-export List, Implementation Detail

### Cluster Execution

**Failure Detector (故障検出器)**:
Cluster Member (クラスタメンバー) が available / unavailable に見えるかを観測する cluster 実行上の関心事。Availability Evidence (可用性観測証拠) を生成するが、それ自体は Member Removal (メンバー除去) を決定しない。
_Avoid_: Downing Strategy, Membership Removal Policy

**Failure Detector Configuration (故障検出器設定)**:
Availability Evidence (可用性観測証拠) の観測方法を調整する Cluster Configuration (クラスタ設定) の一部。コード上の設定型名は `FailureDetectorConfig` とし、主契約は Failure Detector (故障検出器) の観測挙動を設定することであり、任意の Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を公開することではない。
_Avoid_: Failure Detector Implementation Choice

**Availability Evidence (可用性観測証拠)**:
Cluster Member (クラスタメンバー) が reachable / unreachable に見えるという観測情報。Membership Decision (メンバーシップ判断) や Downing Decision (ダウン判断) の入力にはなるが、それ自体は member を remove / down する決定ではない。
_Avoid_: Downing Decision, Member Removal

**Cluster Configuration (クラスタ設定)**:
cluster extension の install / start 時に渡す利用者向け configuration。Membership Coordinator (メンバーシップ調停器) の外から見えるべき Cluster Operational Contract (クラスタ運用契約) を所有する。
_Avoid_: Coordinator-only Settings

**Membership Coordination Policy (メンバーシップ調停ポリシー)**:
Topology Input (トポロジ入力) と Availability Evidence (可用性観測証拠) を Membership State Transition (メンバーシップ状態遷移) へ変換する規則。Availability Evidence (可用性観測証拠) を生成する Failure Detector Configuration (故障検出器設定) とは分離する。
_Avoid_: Failure Detector Configuration

**Join Compatibility (参加互換性)**:
新しい Cluster Member (クラスタメンバー) を受け入れる前に、Cluster Operational Contract (クラスタ運用契約) が既存 member と一致しているかを確認すること。Availability Evidence (可用性観測証拠) の前提が揺れる Failure Detector Configuration (故障検出器設定) は、この確認対象に含める。
_Avoid_: Best-effort Configuration Drift

**Compatibility Mismatch Reason (互換性不一致理由)**:
Join Compatibility (参加互換性) が失敗したときに、どの Cluster Operational Contract (クラスタ運用契約) が一致しなかったかを説明する診断情報。運用者が設定差分を特定するための情報であり、互換性判定そのものではない。
_Avoid_: Generic Mismatch (一般的な不一致), Opaque Rejection (不透明な拒否)

**Cluster Member (クラスタメンバー)**:
cluster に参加し、membership state を持つノード。Availability Evidence (可用性観測証拠) と Membership State Transition (メンバーシップ状態遷移) の対象になる。
_Avoid_: Node without membership state

**Member Identity (メンバー識別)**:
Cluster Member (クラスタメンバー) の incarnation を address と uid の組で区別する識別契約。authority 文字列だけで同一 member とみなすものではなく、再起動後の同一 address 再利用を別 member として扱うための語彙である。
_Avoid_: Authority-only Identity, Node Address, Host Port

**Member Removal (メンバー除去)**:
Cluster Member (クラスタメンバー) を active membership view から外すこと。Availability Evidence (可用性観測証拠) の生成とは別の判断である。
_Avoid_: Availability Evidence

**Cluster Operational Contract (クラスタ運用契約)**:
cluster 内の member 間で揃える必要がある運用上の前提。Join Compatibility (参加互換性) は、この前提が一致しているかを確認する。
_Avoid_: Best-effort settings

**Membership Coordinator (メンバーシップ調停器)**:
Topology Input (トポロジ入力) と Availability Evidence (可用性観測証拠) を受け取り、Membership State Transition (メンバーシップ状態遷移) を調停する cluster コンポーネント。
_Avoid_: Failure Detector, Downing Strategy

**Topology Input (トポロジ入力)**:
cluster provider や discovery から届く、Cluster Member (クラスタメンバー) の参加・離脱・到達性に関する入力。Membership Coordinator (メンバーシップ調停器) が membership view の更新材料として扱う。
_Avoid_: Membership State Transition

**Membership State Transition (メンバーシップ状態遷移)**:
Cluster Member (クラスタメンバー) の membership state が変わること。Topology Input (トポロジ入力) や Availability Evidence (可用性観測証拠) から導かれるが、それらとは別の結果である。
_Avoid_: Availability Evidence, Topology Input

**Weakly Up Member (暫定参加メンバー)**:
join 中に暫定的に active view へ入った Cluster Member (クラスタメンバー)。通常の Up member と区別され、Split Brain Resolution (スプリットブレイン解決) や Downing Decision (ダウン判断) を実行する理由そのものではない。
_Avoid_: Up Member, Failure Evidence, Downing Input

**Membership Decision (メンバーシップ判断)**:
Membership Coordinator (メンバーシップ調停器) が membership view をどう更新するかの判断。Downing Decision (ダウン判断) とは分離する。
_Avoid_: Downing Decision

**Reachability Matrix (到達性マトリクス)**:
observer member が subject member を reachable / unreachable / terminated と観測したレコードを、version 付きで保持する reachability evidence の構造。Membership State Transition (メンバーシップ状態遷移) や Downing Decision (ダウン判断) ではなく、判断の入力になる観測情報である。
_Avoid_: Membership State, Downing Decision, Failure Detector

**Downing Decision (ダウン判断)**:
Cluster Member (クラスタメンバー) を down として扱うかを決める判断。Availability Evidence (可用性観測証拠) を入力にできるが、Failure Detector (故障検出器) 自体の責務ではない。
_Avoid_: Availability Evidence, Failure Detector

**Split Brain Resolution (スプリットブレイン解決)**:
network partition などで cluster view が分断されたとき、membership snapshot と Availability Evidence (可用性観測証拠) からどの partition / member を keep または down するかを評価する policy contract。Failure Detector (故障検出器) や Membership Coordinator (メンバーシップ調停器) ではない。
_Avoid_: Failure Detector, Membership Coordinator, Reachability Matrix

**Failure Detector Algorithm Selection (故障検出器アルゴリズム選択)**:
使用する Failure Detector (故障検出器) のアルゴリズムを選ぶこと。現時点の Failure Detector Configuration (故障検出器設定) は、この選択を主契約にしない。
_Avoid_: Failure Detector Configuration

**Cluster Configuration Validation (クラスタ設定検証)**:
Cluster Configuration (クラスタ設定) が Cluster Operational Contract (クラスタ運用契約) として成立するかを確認すること。builder API ではなく、install / start 境界でまとめて実行する。
_Avoid_: Builder Validation (builder 検証)

**Member Ordering (メンバー順序)**:
Cluster Member (クラスタメンバー) の決定的な全順序の公開契約。参加の古さに基づく age ordering を含み、Downing Decision (ダウン判断) の KeepOldest や将来の singleton 選出が同じ順序結果を参照するための観測契約であって、選出や配置の決定そのものではない。
_Avoid_: Oldest Election, Singleton Selection

**Shutdown Progress Event (シャットダウン進行イベント)**:
Cluster Member (クラスタメンバー) の shutdown 準備の開始・完了にあたる Membership State Transition (メンバーシップ状態遷移) を購読者へ知らせるイベント。full cluster shutdown を開始する command ではない。
_Avoid_: Full Cluster Shutdown Command

**Data Center Reachability (データセンター到達性)**:
cross-DC の Availability Evidence (可用性観測証拠) から導かれる、data center 単位の reachable / unreachable の観測。member 単位の到達性とは区別され、それ自体は Downing Decision (ダウン判断) や Member Removal (メンバー除去) を実行しない。
_Avoid_: Member-level Reachability, Downing Decision

**Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド)**:
cluster lifecycle の主要な遷移を、トレース出力で機械的に解析可能にするための、遷移種別ごとの構造化フィールド名の公開契約。フィールド名の語彙を所有するのであって、出力先や tracing 実装の選択ではない。
_Avoid_: Log Output Format, Tracing Backend Choice

**Gossip Protocol (ゴシッププロトコル)**:
Cluster Member (クラスタメンバー) 間で membership state、seen digest、tombstone、heartbeat evidence を交換し、membership convergence を説明可能にする dissemination contract。Topic Registry (トピックレジストリ) や Distributed Data (分散データ) の replication protocol ではない。
_Avoid_: PubSub Gossip, CRDT Replication, Wire Serialization

**Cluster Heartbeat Protocol (クラスタハートビートプロトコル)**:
Cluster Member (クラスタメンバー) 間で liveness evidence を request / response として交換し、Failure Detector (故障検出器) と Reachability Matrix (到達性マトリクス) の入力を作る protocol。transport keepalive や Downing Decision (ダウン判断) そのものではない。
_Avoid_: Transport Keepalive, Downing Decision, Gossip Merge

**Discovery Normalization (ディスカバリ正規化)**:
static seed node や discovery backend 固有の endpoint 結果を、provider-neutral な Topology Input (トポロジ入力) へ変換する、provider boundary の契約。backend metadata を placement policy や Membership Decision (メンバーシップ判断) へ直接渡すものではない。
_Avoid_: Provider-specific Policy, Placement Decision, Discovery Metadata

**PubSub Mediator Protocol (PubSubメディエータプロトコル)**:
cluster pub-sub の register / subscribe / publish / query と path delivery を、topic registry と delivery intent として扱う protocol。EventBus Classification (イベントバス分類)、Distributed Data (分散データ)、Gossip Protocol (ゴシッププロトコル) とは別の配信契約である。
_Avoid_: EventBus, Distributed Data, Gossip Protocol

**Topic Registry (トピックレジストリ)**:
PubSub Mediator Protocol (PubSubメディエータプロトコル) の中で、topic subscription と path registration を owner / version / tombstone 付きで保持する replicated registry。Distributed Data (分散データ) の CRDT store や actor EventStream (イベントストリーム) ではない。
_Avoid_: Distributed Data, EventStream, Membership State

**Grain (グレイン)**:
cluster 上で identity によって参照され、placement / activation の対象になる virtual actor entity。Actor Cell (アクターセル) や Cluster Member (クラスタメンバー) ではなく、cluster grain runtime が配送対象として扱う entity である。
_Avoid_: Actor Cell, Cluster Member, Raw ActorRef

**Sharding Extractor (シャーディング抽出器)**:
message または envelope から entity id と shard id を導出する、差し替え可能な Grain (グレイン) 配送契約。placement、rebalance、activation / passivation の決定規則ではない。
_Avoid_: Placement Strategy, Rebalance Policy, Message Serialization

**Grain Readiness (グレイン準備状態)**:
Grain (グレイン) の runtime が traffic を受けられるかを、membership state、placement coordination、kind registration から導出する readiness contract。process liveness や HTTP probe endpoint の実装ではない。
_Avoid_: Process Liveness, Probe Endpoint, Placement Decision

**Cluster Singleton (クラスタシングルトン)**:
cluster 全体で 1 つだけ動く actor を保つための manager / proxy / handover に関する cluster 実行契約。Member Ordering (メンバー順序) を利用できるが、Member Ordering (メンバー順序) そのものや generic leader election ではない。
_Avoid_: Leader Election, Member Ordering, Single Local Actor

**Distributed Data (分散データ)**:
cluster 内で replicated data を扱うためのドメイン。CRDT (収束データ型)、Replicator (レプリケータ)、consistency level の語彙を含むが、membership gossip や PubSub Mediator Protocol (PubSubメディエータプロトコル) とは別の領域である。
_Avoid_: Membership Gossip, PubSub Mediator Protocol, Durable Persistence

**CRDT (収束データ型)**:
複数の replica の更新を merge によって収束させる replicated data 型。Replicator (レプリケータ) の実行 loop や wire serialization ではなく、merge law と node contribution の意味を所有するデータ契約である。
_Avoid_: Replicator Runtime, Wire Serialization, Arbitrary Data Model

**Replicator (レプリケータ)**:
Distributed Data (分散データ) の CRDT (収束データ型) 更新、read / write consistency、subscription notification を cluster 内で伝播する runtime コンポーネント。CRDT 型そのものや Gossip Protocol (ゴシッププロトコル) ではない。
_Avoid_: CRDT Type, Gossip Protocol, PubSub Mediator Protocol

**Cluster Compatibility Baseline (クラスタ互換性ベースライン)**:
Join Compatibility (参加互換性) と downstream cluster specs が共有する、比較対象 key、比較対象外 key、互換性理由の基礎契約。Split Brain Resolution (スプリットブレイン解決) や Discovery Normalization (ディスカバリ正規化) の実行体ではない。
_Avoid_: Full Cluster Parity, Downing Execution, Discovery Execution

**Downing Strategy (ダウン戦略)**:
Split Brain Resolution (スプリットブレイン解決) で membership snapshot と Availability Evidence (可用性観測証拠) から keep / down / defer の判断規則を選ぶ policy identity。Downing Decision (ダウン判断) の結果そのものや Failure Detector (故障検出器) ではない。
_Avoid_: Downing Decision, Failure Detector, Membership State

**Lease Majority (リース多数派)**:
Split Brain Resolution (スプリットブレイン解決) で多数派 partition が外部 lease を取得できた場合だけ keep decision を成立させる strategy contract。lease backend の実装や network I/O ではなく、lease acquisition outcome を Downing Strategy (ダウン戦略) に反映する語彙である。
_Avoid_: Lease Backend, Network Lock, Downing Execution

**Cluster Message Serialization (クラスタメッセージ直列化)**:
cluster protocol payload を actor-core serialization metadata と cluster payload kind によって wire bridge へ渡す contract。Gossip Protocol (ゴシッププロトコル) や PubSub Mediator Protocol (PubSubメディエータプロトコル) の意味論を実行するものではない。
_Avoid_: Gossip Merge, PubSub Delivery, Protobuf Compatibility

**Cluster Wire Frame (クラスタワイヤフレーム)**:
Cluster Message Serialization (クラスタメッセージ直列化) の payload kind、serializer id、manifest、bytes を versioned wire 形状として運ぶ std/wire 境界の契約。transport lifecycle や actor-core serializer registry そのものではない。
_Avoid_: Transport Lifecycle, Serializer Registry, Protocol Semantics

**Distributed Data Key (分散データキー)**:
Distributed Data (分散データ) の CRDT (収束データ型) 値を型安全に識別する key contract。actor path や Topic Registry (トピックレジストリ) の key ではなく、Replicator (レプリケータ) が扱う replicated value identity である。
_Avoid_: Actor Path, Topic Registry Key, Raw String Id

**Distributed Data Consistency (分散データ整合性)**:
Distributed Data (分散データ) の read / write 操作が Local、quorum、all などのどの範囲の replica 応答を要求するかを表す consistency contract。CRDT (収束データ型) の merge law や durable persistence ではない。
_Avoid_: CRDT Merge Law, Durable Persistence, Membership Quorum

**Version Vector (バージョンベクター)**:
replicated data の dot / causal history を比較し、observed-remove や pruning の因果関係を表す causal clock。membership 用の VectorClock や wall-clock timestamp ではない。
_Avoid_: Membership VectorClock, Wall Clock, Timestamp

**Observed-Remove CRDT (観測除去CRDT)**:
追加時に観測した causal dots を削除時に取り消すことで、並行 add / remove を収束的に扱う CRDT (収束データ型) の family。単純な grow-only collection や last-writer-wins value ではない。
_Avoid_: Grow-only Collection, Last-writer-wins Value, Plain Set
