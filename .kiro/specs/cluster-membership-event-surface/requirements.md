# 要件定義

## はじめに

fraktor-rs の cluster 実装者と運用者は、cluster membership の観測面を通じて、Cluster Member (クラスタメンバー) の順序、graceful shutdown の進行、data center 単位の到達性変化を観測したい。

現状、shutdown 系の membership 状態（PreparingForShutdown / ReadyForShutdown）と Membership State Transition (メンバーシップ状態遷移) の規則は実装済みだが、shutdown 進行を区別して観測できるイベントがない。最古メンバーの判定は内部実装に閉じており、Member Ordering (メンバー順序) の公開契約がない。Cross-DC の Availability Evidence (可用性観測証拠) はあるが、Data Center Reachability (データセンター到達性) の変化をイベントとして観測できない。cluster lifecycle の主要遷移を機械的に解析可能な形で追跡する Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) の契約もない。

この feature は、既存の Membership State Transition (メンバーシップ状態遷移) の規則を変更せず、観測面（順序契約・イベント・トレースフィールド契約)だけを追加する。

## 境界コンテキスト

- **対象範囲**: Member Ordering (メンバー順序) と age ordering の公開契約、shutdown 進行イベント、Data Center Reachability (データセンター到達性) イベント、Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) 契約。
- **対象外**: full cluster shutdown を開始する command path、Downing Decision (ダウン判断) と Member Removal の実行、Split Brain Resolver の runtime loop、singleton の選出実行、gossip / heartbeat protocol 本体の変更、イベント payload の wire serialization。
- **隣接システム／スペックへの期待**: cluster-membership-reachability-model（完了済み）の membership view と Availability Evidence (可用性観測証拠) を入力として読む。cluster-downing-sbr-decision-model の KeepOldest 判定は、この feature が公開する Member Ordering (メンバー順序) と同じ順序結果になることを期待する。

## 要件

### 要件 1: Member Ordering (メンバー順序) の公開契約

**目的:** cluster 実装者として、downing 判断や将来の singleton 選出が同じ順序結果を参照できるように、Cluster Member (クラスタメンバー) の決定的な順序を公開契約として使いたい

#### 受け入れ基準

1. membership view の Cluster Member (クラスタメンバー) 集合に順序が要求されたとき、fraktor-rs cluster は決定的な全順序で member を並べなければならない。
2. fraktor-rs cluster は常に、同一の membership view に対する順序要求へ同一の並びを返さなければならない。
3. age ordering が要求されたとき、fraktor-rs cluster は cluster への参加が古い順に Cluster Member (クラスタメンバー) を並べなければならない。
4. 参加の古さが同等の Cluster Member (クラスタメンバー) が複数存在する場合、fraktor-rs cluster は決定的な tie-break で順序を一意に解決しなければならない。
5. 最古メンバーの特定が要求されたとき、fraktor-rs cluster は age ordering の先頭にあたる Cluster Member (クラスタメンバー) を返さなければならない。
6. membership view が空の場合、fraktor-rs cluster は最古メンバーが存在しないことを明示的な結果として返さなければならない。

### 要件 2: Shutdown Progress Event (シャットダウン進行イベント) の観測

**目的:** cluster 運用者として、cluster 全体の graceful shutdown がどこまで進んだかを購読者から観測したい

#### 受け入れ基準

1. Cluster Member (クラスタメンバー) が PreparingForShutdown 状態へ遷移したとき、fraktor-rs cluster は shutdown 準備開始を表すイベントを cluster イベント購読者へ配信しなければならない。
2. Cluster Member (クラスタメンバー) が ReadyForShutdown 状態へ遷移したとき、fraktor-rs cluster は shutdown 準備完了を表すイベントを cluster イベント購読者へ配信しなければならない。
3. shutdown 進行イベントを配信するとき、fraktor-rs cluster は対象 Cluster Member (クラスタメンバー) の識別情報を含めなければならない。
4. shutdown 進行イベントを配信する場合、fraktor-rs cluster は shutdown 準備開始と shutdown 準備完了を購読者が区別できる形で配信しなければならない。
5. gossip 経由で remote の Cluster Member (クラスタメンバー) の shutdown 系状態遷移を観測したとき、fraktor-rs cluster は local 起点の遷移と同様に shutdown 進行イベントを配信しなければならない。

### 要件 3: Data Center Reachability (データセンター到達性) の観測

**目的:** cluster 運用者として、multi-DC 構成において data center 単位の到達性変化を member 単位の到達性とは別に観測したい

#### 受け入れ基準

1. cross-DC の観測対象である data center の全ての観測対象 Cluster Member (クラスタメンバー) について Availability Evidence (可用性観測証拠) が unreachable を示したとき、fraktor-rs cluster は当該 data center が unreachable になったことを表すイベントを配信しなければならない。
2. unreachable と判定済みの data center で少なくとも 1 つの観測対象 Cluster Member (クラスタメンバー) が再び reachable と観測されたとき、fraktor-rs cluster は当該 data center が reachable に戻ったことを表すイベントを配信しなければならない。
3. Data Center Reachability (データセンター到達性) イベントを配信するとき、fraktor-rs cluster は対象 data center の識別子を含めなければならない。
4. fraktor-rs cluster は常に、自 node が属する data center を Data Center Reachability (データセンター到達性) 判定の対象外として扱わなければならない。
5. Data Center Reachability (データセンター到達性) イベントを配信する場合、fraktor-rs cluster は member 単位の到達性イベントと購読者が区別できる形で配信しなければならない。
6. Data Center Reachability (データセンター到達性) を判定する場合、fraktor-rs cluster は判定結果を理由に Downing Decision (ダウン判断) や Member Removal を実行してはならない。

### 要件 4: Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) 契約

**目的:** cluster 運用者として、cluster lifecycle の主要遷移を構造化フィールドで機械的に解析できるようにしたい

#### 受け入れ基準

1. cluster lifecycle の主要遷移（join / up / leave / removal に相当する Membership State Transition (メンバーシップ状態遷移)、shutdown 進行、Data Center Reachability (データセンター到達性) 変化）を含む場合、fraktor-rs cluster は遷移種別ごとに一意なトレースフィールド名の契約を定義しなければならない。
2. Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) 契約を定義する場合、fraktor-rs cluster は対象 Cluster Member (クラスタメンバー) の識別情報を表すフィールド名を含めなければならない。
3. fraktor-rs cluster は常に、Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) 契約を単一の公開定義として提供しなければならない。
4. std 層の実装が cluster lifecycle の遷移をトレース出力するとき、fraktor-rs cluster は定義済みの Cluster Lifecycle Trace Field (クラスタライフサイクル トレースフィールド) 契約のフィールド名を使わなければならない。

### 要件 5: Scope Boundary (スコープ境界)

**目的:** cluster 実装者として、この feature が観測面の追加に閉じていることを確認し、状態遷移規則や判断責務と混同しないようにしたい

#### 受け入れ基準

1. この feature の観測面を追加する場合、fraktor-rs cluster は既存の Membership State Transition (メンバーシップ状態遷移) の規則を変更してはならない。
2. shutdown 進行イベントを提供する場合、fraktor-rs cluster は full cluster shutdown を開始する command を公開契約に追加してはならない。
3. Member Ordering (メンバー順序) を提供する場合、fraktor-rs cluster は singleton の選出や placement の決定を実行してはならない。
4. 観測面のイベントを追加する場合、fraktor-rs cluster は既存のイベント購読契約で配信済みのイベント種別を削除してはならない。
