# fraktor-rs Actor System Context

fraktor-rs は、actor / remote / cluster / stream / persistence の各ドメインに対して、移植可能な実行契約を定義する Rust actor system である。

## Language

### Actor Execution

**Actor Cell (アクターセル)**:
actor instance の mailbox dispatch、lifecycle、supervision、children relation、DeathWatch (死亡監視)、context side effects を調停する actor-core-kernel の実行コンテナ。Actor (アクター) の user behavior や ActorRef (アクター参照) の address identity とは別の実行境界である。
_Avoid_: Runtime, Actor, ActorRef, Mailbox Owner

**Actor Cell Facet (アクターセルファセット)**:
Actor Cell (アクターセル) の同一型実装を dispatch、lifecycle、fault handling、children、DeathWatch (死亡監視)、Receive Timeout (受信タイムアウト) などの責務単位で追跡するための内部実装境界。public trait や利用者向け API を増やすための境界ではない。
_Avoid_: Runtime, Public Facet API, Trait Facet, ActorCell Subclass

**Actor System State (アクターシステム状態)**:
actor system scoped state を既存 accessor 経由で扱う façade。実行補助、dispatcher / mailbox、event / logging、guardian / cells、remote / provider、scheduler / lifecycle などの System State Registry (システム状態レジストリ) を束ねるが、それぞれの subsystem behavior を直接所有する概念ではない。
_Avoid_: Runtime, Global State, God Object, Shared State Bag

**System State Registry (システム状態レジストリ)**:
Actor System State (アクターシステム状態) の内側で、dispatcher / mailbox、event / logging、guardian / cells など単一 subsystem の state ownership を担う private registry。外部 crate に公開する registry handle ではなく、既存 façade から委譲される内部境界である。
_Avoid_: Runtime, Public Registry Handle, Shared Global Registry, Service Locator

**DeathWatch (死亡監視)**:
ある actor が別の actor の termination を観測し、終了通知を受け取る actor 実行上の観測契約。Lifecycle Event (ライフサイクルイベント) の発行や child registry の所有とは区別する。
_Avoid_: Runtime, Lifecycle Event, Child Registry, Failure Detection

**Receive Timeout (受信タイムアウト)**:
actor が一定期間 timeout に影響する user message を受信しなかったときに timeout signal を届ける実行契約。scheduler の停止や mailbox の idle 状態そのものではなく、actor behavior に見える inactivity signal である。
_Avoid_: Runtime, Scheduler Timeout, Mailbox Idle Timeout, Shutdown Timeout

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
new Cluster Member (クラスタメンバー) を受け入れる前に、Cluster Operational Contract (クラスタ運用契約) が既存 member と一致しているかを確認すること。Availability Evidence (可用性観測証拠) の前提が揺れる Failure Detector Configuration (故障検出器設定) は、この確認対象に含める。
_Avoid_: Best-effort Configuration Drift

**Compatibility Mismatch Reason (互換性不一致理由)**:
Join Compatibility (参加互換性) が失敗したときに、どの Cluster Operational Contract (クラスタ運用契約) が一致しなかったかを説明する診断情報。運用者が設定差分を特定するための情報であり、互換性判定そのものではない。
_Avoid_: Generic Mismatch (一般的な不一致), Opaque Rejection (不透明な拒否)

**Cluster Member (クラスタメンバー)**:
cluster に参加し、membership state を持つ node。Availability Evidence (可用性観測証拠) と Membership State Transition (メンバーシップ状態遷移) の対象になる。
_Avoid_: Node without membership state

**Member Removal (メンバー除去)**:
Cluster Member (クラスタメンバー) を active membership view から外すこと。Availability Evidence (可用性観測証拠) の生成とは別の判断である。
_Avoid_: Availability Evidence

**Cluster Operational Contract (クラスタ運用契約)**:
cluster 内の member 間で揃える必要がある運用上の前提。Join Compatibility (参加互換性) は、この前提が一致しているかを確認する。
_Avoid_: Best-effort settings

**Membership Coordinator (メンバーシップ調停器)**:
Topology Input (トポロジ入力) と Availability Evidence (可用性観測証拠) を受け取り、Membership State Transition (メンバーシップ状態遷移) を調停する cluster component。
_Avoid_: Failure Detector, Downing Strategy

**Topology Input (トポロジ入力)**:
cluster provider や discovery から届く、Cluster Member (クラスタメンバー) の参加・離脱・到達性に関する入力。Membership Coordinator (メンバーシップ調停器) が membership view の更新材料として扱う。
_Avoid_: Membership State Transition

**Membership State Transition (メンバーシップ状態遷移)**:
Cluster Member (クラスタメンバー) の membership state が変わること。Topology Input (トポロジ入力) や Availability Evidence (可用性観測証拠) から導かれるが、それらとは別の結果である。
_Avoid_: Availability Evidence, Topology Input

**Membership Decision (メンバーシップ判断)**:
Membership Coordinator (メンバーシップ調停器) が membership view をどう更新するかの判断。Downing Decision (ダウン判断) とは分離する。
_Avoid_: Downing Decision

**Downing Decision (ダウン判断)**:
Cluster Member (クラスタメンバー) を down として扱うかを決める判断。Availability Evidence (可用性観測証拠) を入力にできるが、Failure Detector (故障検出器) 自体の責務ではない。
_Avoid_: Availability Evidence, Failure Detector

**Failure Detector Algorithm Selection (故障検出器アルゴリズム選択)**:
使用する Failure Detector (故障検出器) の algorithm を選ぶこと。現時点の Failure Detector Configuration (故障検出器設定) は、この選択を主契約にしない。
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
cluster lifecycle の主要遷移をトレース出力で機械的に解析可能にするための、遷移種別ごとの構造化フィールド名の公開契約。フィールド名の語彙を所有するのであって、出力先や tracing 実装の選択ではない。
_Avoid_: Log Output Format, Tracing Backend Choice
