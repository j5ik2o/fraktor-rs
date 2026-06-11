# 要件定義

## はじめに

Cluster Singleton（cluster 全体で 1 つだけ動く actor を保証する機構）の設定・検証・参加互換性・観測の契約を定義する。Phase 3 で singleton manager / proxy の runtime（oldest 選出・handover 状態機械・メッセージバッファリング）を実装する前に、設定契約を独立した spec として固めることで、runtime 実装と設定設計が同じレビュースコープに混ざることを防ぐ。

対象は Pekko の `ClusterSingletonManagerSettings`（classic）、`ClusterSingletonProxySettings`、typed `ClusterSingletonSettings` に相当する設定項目群、install / start 境界での Cluster Configuration Validation (クラスタ設定検証)、Join Compatibility (参加互換性) への組み込み、ActorSystem 構築時の統合点、および handover が進まない状態（stuck）を検知する観測契約である。`configure-cluster-failure-detector` で確立した「設定型 → 検証 → 互換キー → 統合」のパターンを踏襲する。

## 境界コンテキスト

- **対象範囲**: singleton manager / proxy / typed 統合の設定契約、install / start 境界での設定検証、Join Compatibility (参加互換性) への組み込みと Compatibility Mismatch Reason (互換性不一致理由) の生成、ActorSystem 構築時の設定統合点、stuck 検知の観測契約（検知条件と通知内容の定義）
- **対象外**: singleton manager / proxy の runtime 状態機械（oldest 選出、handover 実行、メッセージバッファリング、location 追跡）、lease backend の実装、coordinated shutdown との handover 連携、singleton の配置決定ロジック（Member Ordering (メンバー順序) 契約は cluster-membership-event-surface が所有済み）
- **隣接システム／スペックへの期待**: cluster-active-compatibility-baseline が所有する互換キーの基盤（カタログと合成）に新キーを追加できること。configure-cluster-failure-detector が確立した設定契約の利用パターンが先行例として参照できること。Phase 3 の runtime spec は本契約の設定・観測契約を参照するだけで着手できること

## 要件

### 要件 1: Singleton Manager 設定契約

**目的:** クラスタ運用者として、singleton runtime の実装に先立って manager の動作パラメータを固定するために、singleton manager の設定契約が欲しい

#### 受け入れ基準

1. cluster 拡張は常に、singleton manager 設定として singleton 名、対象 role（任意）、removal margin、handover リトライ間隔、lease 設定スロット（任意）を保持できなければならない
2. 利用者が設定値を指定しない場合、cluster 拡張は既定値（singleton 名 "singleton"、role 制約なし、removal margin 未指定、lease なし）を適用しなければならない
3. removal margin が未指定の場合、cluster 拡張は「downing 側の removal margin に従う」状態として明示済みの値と区別できる形で保持しなければならない
4. lease 設定スロットを含む場合、cluster 拡張は lease 実装の識別名と lease リトライ間隔を設定として保持しなければならない

### 要件 2: Singleton Proxy 設定契約

**目的:** クラスタ運用者として、singleton へアクセスする側の動作パラメータを固定するために、singleton proxy の設定契約が欲しい

#### 受け入れ基準

1. cluster 拡張は常に、singleton proxy 設定として singleton 名、対象 role（任意）、data center（任意）、identification 間隔、buffer size を保持できなければならない
2. 利用者が設定値を指定しない場合、cluster 拡張は既定値（singleton 名 "singleton"、role 制約なし、data center 制約なし）を適用しなければならない
3. buffer size にゼロが指定された場合、cluster 拡張は「バッファリングなし」を意味する有効な構成として受け入れなければならない

### 要件 3: typed 統合設定契約

**目的:** typed API の利用者として、manager / proxy の設定を個別に組み立てる手間を省くために、両者の項目を一括指定できる統合設定契約が欲しい

#### 受け入れ基準

1. cluster 拡張は常に、role、data center、identification 間隔、removal margin、handover リトライ間隔、buffer size、lease 設定スロットを単一の統合設定として一括指定できなければならない
2. 統合設定から singleton 名を与えて manager 設定を導出する場合、cluster 拡張は対応する設定項目の値を変化させずに引き継がなければならない
3. 統合設定から singleton 名を与えて proxy 設定を導出する場合、cluster 拡張は対応する設定項目の値を変化させずに引き継がなければならない
4. 利用者が設定値を指定しない場合、cluster 拡張は要件 1・要件 2 と同じ既定値を適用しなければならない

### 要件 4: 設定検証

**目的:** クラスタ運用者として、不成立な singleton 設定で cluster が起動してしまう事故を防ぐために、install / start 境界での設定検証が欲しい

#### 受け入れ基準

1. install / start 境界の Cluster Configuration Validation (クラスタ設定検証) が起きたとき、cluster 拡張は singleton 設定の検証を既存の設定検証と同じ境界でまとめて実行しなければならない
2. buffer size が許容範囲（0 以上 10000 以下）の外にある場合、cluster 拡張は構成を拒否し、原因項目を特定できる理由を提示しなければならない
3. handover リトライ間隔または identification 間隔がゼロ以下の場合、cluster 拡張は構成を拒否し、原因項目を特定できる理由を提示しなければならない
4. singleton 名が空文字の場合、cluster 拡張は構成を拒否し、原因項目を特定できる理由を提示しなければならない
5. lease 設定スロットを含む場合、lease 実装の識別名が空文字、または lease リトライ間隔がゼロ以下のとき、cluster 拡張は構成を拒否しなければならない
6. すべての設定値が成立する場合、cluster 拡張は検証を通過させ、install を継続しなければならない

### 要件 5: Join Compatibility への組み込み

**目的:** クラスタ運用者として、singleton の挙動前提が member 間でずれた構成のまま cluster が混在することを防ぐために、singleton 設定を Cluster Operational Contract (クラスタ運用契約) の確認対象に含めたい

#### 受け入れ基準

1. 新規 Cluster Member (クラスタメンバー) の Join Compatibility (参加互換性) 確認が起きたとき、cluster 拡張は singleton 設定を既存 member との一致確認の対象に含めなければならない
2. singleton 設定が既存 member と一致しない場合、cluster 拡張はどの設定項目が不一致かを特定できる Compatibility Mismatch Reason (互換性不一致理由) を生成しなければならない
3. singleton 設定が既存 member と一致する場合、cluster 拡張は singleton 設定を理由とした参加拒否を発生させてはならない

### 要件 6: ActorSystem 構築時の統合点

**目的:** cluster 利用者として、singleton 設定を他の cluster 設定と同じ入口から渡すために、ActorSystem 構築時の統合点が欲しい

#### 受け入れ基準

1. ActorSystem の構築が起きたとき、cluster 拡張は singleton 設定を含む Cluster Configuration (クラスタ設定) を install 境界で受け取れなければならない
2. 利用者が singleton 設定を指定しない場合、cluster 拡張は既定値の singleton 設定で install を成立させなければならない
3. 既存の cluster 設定だけを使う構成の場合、cluster 拡張は本契約の追加前と同じ挙動で install を成立させなければならない

### 要件 7: Stuck 検知の観測契約

**目的:** クラスタ運用者として、singleton の handover が進まない障害を把握するために、stuck 状態の検知条件と通知内容の契約が欲しい

#### 受け入れ基準

1. cluster 拡張は常に、stuck 状態の検知条件を「handover の進行がリトライ上限を超えても完了しない状態」として定義し、リトライ上限を removal margin と handover リトライ間隔から決定的に導出できなければならない
2. cluster 拡張は常に、stuck 通知の内容として singleton 名、停滞の局面（oldest への昇格待ち、または handover 実行中）、観測時刻を定義しなければならない
3. stuck 通知が発行された場合、購読者は他の cluster イベントと区別して stuck 通知だけを識別できなければならない
4. stuck 通知が発行された場合でも、cluster 拡張は通知を契機とした Membership State Transition (メンバーシップ状態遷移) や Downing Decision (ダウン判断) を実行してはならない

### 要件 8: 既存挙動の維持と範囲の限定

**目的:** cluster 利用者として、既存機能の安定を保つために、本契約の追加が既存挙動を変えないことと、runtime を先取りしないことを保証したい

#### 受け入れ基準

1. cluster 拡張は常に、既存の設定検証および既存の互換キーによる参加互換性判定の挙動を変えずに維持しなければならない
2. cluster 拡張は本契約の範囲で singleton manager / proxy の runtime 動作（oldest 選出、handover 実行、メッセージバッファリング）を提供してはならない
3. cluster 拡張は常に、既存のイベント種別と既存の公開契約を削除・変更せずに維持しなければならない
