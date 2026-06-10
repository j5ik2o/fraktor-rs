# 実装ギャップ分析: cluster-membership-event-surface

分析日: 2026-06-11

## 現状調査サマリー

観測面の追加に必要な判定材料・発行経路は大部分が既存実装に揃っており、ギャップは「公開契約への集約」と「DC 単位の集約判定の新規追加」に集中している。

### 既存資産

| 資産 | 場所 | 関連要件 |
|------|------|----------|
| `NodeRecord::is_older_than`（公開メソッド） | `modules/cluster-core-kernel/src/membership/node_record.rs:80` | 要件 1 |
| `join_version` フィールド（参加時 version） | `node_record.rs`（NodeRecord） | 要件 1 |
| authority 文字列による決定的 tie-break | `node_record.rs:81-85`（`authority_order_key`） | 要件 1 |
| `oldest_authority`（プライベート関数、leader 算出用） | `membership_coordinator.rs:752` | 要件 1 |
| `oldest_active_member`（プライベート関数、KeepOldest 用） | `downing_provider/split_brain_resolver.rs:300` | 要件 1 |
| `role_leaders`（is_older_than + leader-eligible フィルタ） | `membership_coordinator.rs` | 要件 1 |
| `NodeStatus::PreparingForShutdown` / `ReadyForShutdown` と状態遷移 | `node_status.rs`, `membership_coordinator.rs`, `gossip_state_model.rs` | 要件 2 |
| `ClusterEvent` / `ClusterEventType`（12 variant の対応ペア） | `topology/cluster_event.rs`, `topology/cluster_event_type.rs` | 要件 2, 3 |
| `ClusterEvent::MemberStatusChanged`（from/to 付き、coordinator 5 箇所から発行） | `membership_coordinator.rs:186,246,259,557,602` | 要件 2 |
| `MembershipCoordinatorOutcome.member_events: Vec<ClusterEvent>`（発行経路） | `membership_coordinator_outcome.rs:10-21` | 要件 2, 3 |
| `ClusterApi::subscribe`（`ClusterEventType` フィルタ + initial state mode） | `extension/cluster_api.rs:227` | 要件 2, 3 |
| typed `ClusterStateSubscription`（`TypedActorRef<ClusterEvent>` で受信） | `cluster-core-typed/src/cluster_state_subscription.rs` | 要件 2, 3 |
| `CrossDcHeartbeatEvidence`（observer / subject / DC pair / kind） | `membership/cross_dc_heartbeat_evidence.rs:9` | 要件 3 |
| `CrossDcHeartbeat`（観測対象 target の add/remove/retain 管理） | `membership/cross_dc_heartbeat.rs` | 要件 3 |
| `DataCenter` 型と `NodeRecord::data_center` | `membership/data_center.rs` | 要件 3 |
| std 層の tracing 出力（アドホックな field 名: `from`, `target` 等） | `cluster-adaptor-std/src/membership/tokio_gossiper.rs`, `tokio_gossip_transport.rs`, `cluster_provider/aws_ecs_cluster_provider.rs` | 要件 4 |

### 既存パターン・規約（設計が従うべきもの)

- イベント発行: 状態変更は `MembershipCoordinatorOutcome` に蓄積し、`ClusterExtension` が EventStream（`EventStreamEvent::Extension { name: "cluster" }`）へ publish する。
- 購読フィルタ: `ClusterEventType` の variant 単位。variant を追加すると購読者は種別単位で選択的に購読できる。
- 1 公開型 1 ファイル + sibling `_test.rs`、`no_std` + alloc、CQS（判定は `&self` query）。
- typed 層は `ClusterEvent` を型ごと受けるため、variant 追加で構造変更は不要（網羅 match 箇所のみ修正）。

## Requirement-to-Asset Map

| 要件 | 既存資産 | ギャップ | 分類 |
|------|----------|----------|------|
| 1. Member Ordering 公開契約 | `is_older_than` / `join_version` / tie-break は実装済み。ただしペア比較のみ | 集合に対する全順序ソート・age ordering・最古メンバー特定の公開 API がない。`oldest_authority`（coordinator）と `oldest_active_member`（SBR）が同じ判定を**プライベートに重複実装**しており、集約先がない | Missing（集約） |
| 1.4 tie-break | `authority_order_key` による決定的 tie-break 実装済み | 公開契約として明文化されていないのみ | Constraint（既存規則を契約化） |
| 2. Shutdown Progress Event | 状態・遷移・`MemberStatusChanged` 発行経路は実装済み | shutdown 進行を**種別フィルタで選択購読できるイベント**がない。`MemberStatusChanged` は全 status 遷移が混在し、`ClusterEventType` フィルタで shutdown 進行だけを購読できない | Missing（variant 追加） |
| 2.5 remote 由来遷移の観測 | gossip merge による状態反映は `gossip_state_model.rs` にあり | gossip 由来の shutdown 系遷移が `member_events` として発行されるかは未確認 | Unknown（Research Needed） |
| 3. Data Center Reachability | `CrossDcHeartbeatEvidence` / target 管理 / `DataCenter` は実装済み | DC 単位の集約判定（全観測対象 unreachable のラッチ + 復帰判定）と対応する `ClusterEvent` variant が存在しない。新規の状態保持が必要 | Missing（新規ロジック） |
| 3.4 自 DC の除外 | `CrossDcHeartbeat` は DC pair を保持 | 集約判定側で自 DC を対象外にする規則の置き場がない | Missing |
| 4. Trace Field 契約 | std 層の tracing 出力は存在するがアドホック（`from = %addr` 等） | core に field 名の公開契約（定数/型）がなく、std 出力は契約に準拠していない | Missing（契約新設 + 準拠化） |
| 5. Scope Boundary | — | 状態遷移規則は変更しない設計が必要（観測面のみの追加） | Constraint |

## 実装アプローチの選択肢

### Option A: 既存コンポーネントの拡張のみ

`membership` 内の既存ファイル（`membership_coordinator.rs` 等）に ordering 関数を公開昇格し、`ClusterEvent` / `ClusterEventType` に variant を追加、DC 集約判定を `CrossDcHeartbeat` に同居させる。

- ✅ 新規ファイル最小、発行経路の再利用が最大
- ❌ `membership_coordinator.rs` は既に大型（oldest/leader/イベント発行を内包）。ordering 契約を coordinator 内へ足すと肥大が進む
- ❌ DC 集約判定を `CrossDcHeartbeat` に同居させると evidence 生成と判定ラッチの責務が混ざる
- ❌ 1 公開型 1 ファイル lint と衝突しやすい

### Option B: 新規コンポーネントのみ

ordering / DC reachability 集約 / trace field を全て新設モジュールに置き、coordinator には触れない。

- ✅ 責務分離が明確、lint 準拠が容易
- ❌ イベント発行は `MembershipCoordinatorOutcome` 経由が既存契約のため、coordinator 側の接続変更は結局避けられない
- ❌ shutdown 進行イベントは既存の遷移検出（coordinator / gossip model）に依存し、完全な分離は不自然

### Option C: ハイブリッド（推奨）

| 部分 | 方式 |
|------|------|
| Member Ordering 契約 | **新規ファイル**（`membership/` 配下に ordering の公開型/関数群）。`is_older_than` を基礎に集合順序・oldest 特定を提供し、`oldest_authority`（coordinator）と `oldest_active_member`（SBR）の重複実装を新契約の参照へ**集約** |
| shutdown 進行 / DC reachability イベント | **既存拡張**: `ClusterEvent` / `ClusterEventType` に variant 追加、発行は `MembershipCoordinatorOutcome` の既存経路 |
| DC reachability 集約判定 | **新規ファイル**（unreachable ラッチ + 復帰判定の小さな状態モデル）。`CrossDcHeartbeat` evidence を入力に取り、判定結果をイベントとして outcome へ渡す |
| Trace Field 契約 | **新規ファイル**（field 名定数/型の単一公開定義）+ std 層 3 ファイルの出力を契約準拠に修正 |

- ✅ 既存の発行・購読パターンを変えずに variant を増やす（購読者への影響が網羅 match 修正に限定）
- ✅ 重複 oldest 判定の集約で SBR KeepOldest と公開契約の一致（要件 1 の目的）を構造的に保証
- ✅ 新規責務（順序・DC ラッチ・trace 契約）は独立ファイルで lint / レビュー単位が明確
- ❌ coordinator / SBR / gossip model への接続変更が複数箇所に分散（design でファイル単位の影響範囲を明示する必要）

## 工数・リスク評価

| 範囲 | 工数 | リスク | 根拠 |
|------|------|--------|------|
| 要件 1: Member Ordering | S | Low | 判定材料（`is_older_than` / tie-break）が実装済み。公開契約化と 2 箇所の集約のみ |
| 要件 2: Shutdown Progress Event | S–M | Medium | variant 追加と発行は既存パターン。gossip 由来遷移の発行経路が未確認（Research Needed） |
| 要件 3: DC Reachability | M | Medium | 集約ラッチが新規の状態保持。判定条件（全観測対象 unreachable）と `CrossDcHeartbeat` target 管理の接続設計が必要 |
| 要件 4: Trace Field 契約 | S | Low | 契約定義は純粋な定数/型。std 準拠化は 3 ファイルの限定的修正 |
| 全体 | M | Medium | 新規アーキテクチャ変更なし。既存パターンの範囲内で閉じる |

## Research Needed（design フェーズへ持ち越し）

1. **gossip 由来の shutdown 系遷移のイベント発行経路**: `gossip_state_model.rs` の merge で PreparingForShutdown / ReadyForShutdown が反映されたとき、`member_events`（`MemberStatusChanged`）が発行されるか。発行されない場合、要件 2.5 のために merge 結果 → outcome の接続追加が必要。
2. **`ClusterEvent` の網羅 match 箇所の洗い出し**: variant 追加でコンパイルエラーになる箇所（`ClusterEventFilterSubscriber`、`ClusterEventType` 対応、metrics、typed 層、テスト）の全列挙。
3. **ordering 契約とフィルタの直交設計**: `oldest_authority`（フィルタなし）、`role_leaders`（leader-eligible フィルタ）、`oldest_active_member`（active フィルタ）の 3 つの既存利用は対象集合のフィルタが異なる。順序契約は「順序」だけを所有し、フィルタは呼び出し側に残す設計が CQS / 単一責務に整合するか確認。
4. **専用 variant と `MemberStatusChanged` の関係**: shutdown 進行を専用 variant にした場合、同じ遷移で `MemberStatusChanged` も併発するか（二重発行の扱い）。購読者の互換性（要件 5.4: 既存イベント種別の削除禁止）と整合させる。
5. **DC reachability ラッチの観測対象ゼロ件時の挙動**: 観測対象が空になった DC（target retain で消えた場合）の unreachable/reachable 判定をどう扱うか。

## design フェーズへの推奨

- **推奨アプローチ**: Option C（ハイブリッド）
- **重要な設計判断**: (1) shutdown 進行の専用 variant と `MemberStatusChanged` の併発規則、(2) ordering 契約の API 形状（集合ソート vs 比較子提供）、(3) DC ラッチの状態の置き場（`MembershipCoordinatorState` への追加 vs 独立 Shared なしの値型）
- **CONTEXT.md**: 本 feature の新出概念 4 語は反映済み（Member Ordering / Shutdown Progress Event / Data Center Reachability / Cluster Lifecycle Trace Field）

---

# 調査・設計判断ログ（design フェーズ）

調査日: 2026-06-11（design discovery: light / Extension）

## 要約

- **機能**: `cluster-membership-event-surface`
- **ディスカバリー範囲**: 拡張（既存 membership 基盤への観測面追加）
- **主要な発見**:
  - status 遷移イベントは `MembershipCoordinator` の `emit_status_change` ヘルパーに集約されており、gossip 由来（"gossip-dead" 等）の遷移も同経路で `MemberStatusChanged` として発行される。専用イベントの併発はこのヘルパー拡張で全経路をカバーできる
  - `CrossDcHeartbeat` は coordinator に統合されていない独立した pure 状態機械（`update_targets` / `tick` / `handle_response` / `collect_timeouts`）で、std 駆動ループは未配線。DC reachability ラッチも同型の pure 状態機械として設計するのが既存パターンに整合する
  - `ClusterEventType::matches` は `match (self, event)` の網羅 match であり、variant 追加はコンパイルエラーとして全影響箇所を検出できる

## 調査ログ

### gossip 由来の shutdown 系遷移のイベント発行経路（gap analysis Research 1）
- **背景**: 要件 2.5（remote 由来遷移の観測）が既存経路で満たせるかの確認
- **参照した情報源**: `membership_coordinator.rs`（`emit_status_change`、L557/L602 周辺の gossip-dead 遷移、L186/L246/L259 の command 系遷移）
- **発見**: status 遷移の `MemberStatusChanged` 発行は `emit_status_change` に集約されており、local command 系・gossip 系の両方が同じヘルパーを通る
- **含意**: shutdown 進行の専用イベントは `emit_status_change` 内で `to` が shutdown 系のときに併発させる設計で、要件 2.1/2.2/2.5 を単一の変更点でカバーできる

### ClusterEvent の網羅 match 影響範囲（gap analysis Research 2）
- **背景**: variant 追加時の修正範囲の確定（File Structure Plan の入力）
- **参照した情報源**: `rg 'ClusterEvent::'` による非テスト 12 ファイルの列挙、`cluster_event_type.rs` の `matches`
- **発見**: 網羅 match は `ClusterEventType::matches` が中心。typed 層は `TypedActorRef<ClusterEvent>` で型ごと受けるため variant 追加に構造変更不要。その他の使用箇所はほぼ variant 構築側
- **含意**: variant 追加の必須修正は `cluster_event.rs` / `cluster_event_type.rs` のペアに閉じ、残りはコンパイラの網羅性チェックで検出される

### CrossDcHeartbeat の統合状態（gap analysis Research 5 関連）
- **背景**: DC reachability ラッチの状態の置き場と駆動責務の決定
- **参照した情報源**: `cross_dc_heartbeat.rs` の公開 API、coordinator / std からの参照検索（参照なし）
- **発見**: `CrossDcHeartbeat` は driver 側が tick する独立契約で、現状 std 駆動は未配線。evidence（`CrossDcHeartbeatEvidence`: observer / subject / DC pair / kind）と target change が出力される
- **含意**: DC ラッチは `CrossDcHeartbeat` の出力を入力に取る pure 値型とし、Shared 化せず駆動側所有にする。イベント発行接続点は契約（シグネチャ）として定義し、std 駆動ループの配線自体は本 spec の境界外（CrossDcHeartbeat 本体と同じ段階性）

### ordering 重複実装の集約対象（gap analysis Research 3）
- **背景**: 順序契約とフィルタの直交設計の確認
- **参照した情報源**: `oldest_authority`（coordinator、フィルタなし）、`role_leaders`（leader-eligible フィルタ）、`oldest_active_member`（SBR、active フィルタ）
- **発見**: 3 箇所とも `NodeRecord::is_older_than` ベースの比較は同一で、対象集合のフィルタだけが異なる
- **含意**: 順序契約は「比較と順序」だけを所有し、フィルタは呼び出し側に残す直交設計が成立する。集約後も各呼び出し側のフィルタ済み集合に同じ順序関数を適用するだけでよい

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|------|------|--------------|------|
| Option A: 既存拡張のみ | coordinator 等へ直接追加 | 新規ファイル最小 | coordinator 肥大、lint 衝突 | gap analysis で評価済み・不採用 |
| Option B: 新規のみ | 全て新設モジュール | 分離明確 | 発行経路の coordinator 接続は不可避で不自然 | 不採用 |
| Option C: ハイブリッド | variant は既存拡張、順序・ラッチ・trace 契約は新規ファイル | 既存発行/購読パターン維持 + lint 準拠 | 接続変更が複数ファイルに分散 | **採用**（gap analysis の推奨を維持） |

## 設計判断

### 判断: shutdown 進行イベントは専用 variant とし `MemberStatusChanged` と併発する
- **背景**: 要件 2.4（区別可能な配信）と要件 5.4（既存イベント種別の削除禁止）の両立。`ClusterEventType` フィルタで shutdown 進行だけを選択購読できる必要がある
- **検討した代替案**:
  1. `MemberStatusChanged` の to で判別 — フィルタ単位の選択購読ができず要件 2.4 が弱い
  2. 専用 variant のみ（StatusChanged を抑制） — 既存購読者の観測が変わり要件 5.4 違反
- **採用したアプローチ**: `emit_status_change` で `to` が `PreparingForShutdown` / `ReadyForShutdown` のとき、従来の `MemberStatusChanged` に加えて専用 variant を併発する
- **トレードオフ**: 同一遷移で 2 イベントが流れる（購読者は種別フィルタで選択する前提のため許容）
- **フォローアップ**: 併発順序（StatusChanged → 専用）の固定をテストで検証

### 判断: Member Ordering は関数契約として新規ファイルに集約し、フィルタは呼び出し側に残す
- **背景**: 3 箇所のプライベート重複実装の集約と、leader-eligible / active 等の異なるフィルタとの直交性
- **検討した代替案**:
  1. trait 化（`MemberOrdering` trait） — 実装が 1 つしかなく不要な間接化
  2. coordinator のメソッドとして公開 — coordinator 肥大、SBR からの依存方向が不自然
- **採用したアプローチ**: `membership/member_ordering.rs` に pure 関数群（比較・age 整列・oldest 特定）を置き、`is_older_than` を比較の正本として再利用。coordinator / SBR は自前実装を削除してこれを参照
- **根拠**: 単一実装に間接化を持ち込まない（simplification）。比較規則の正本が 1 箇所になり、SBR KeepOldest と公開契約の一致が構造的に保証される（要件 1 の目的）
- **フォローアップ**: 置き換え後に既存の coordinator / SBR テストが無変更で通ることを確認（挙動同値の証明）

### 判断: DC reachability ラッチは Shared 化しない pure 値型とし、std 駆動配線は境界外
- **背景**: `CrossDcHeartbeat` 自体が「契約先行・駆動後続」の段階性で導入済み。要件 3 は判定とイベント形状の契約を要求する
- **検討した代替案**:
  1. coordinator へ統合 — CrossDcHeartbeat が非統合である現状と非対称になり、membership 判断と DC 観測の責務が混ざる
  2. std に直接実装 — no_std で検証可能な判定ロジックが std へ漏れる
- **採用したアプローチ**: core に unreachable ラッチ + 復帰判定の値型を新設し、`CrossDcHeartbeatEvidence` / target change を入力、DC reachability 遷移を出力とする。駆動とEventStream への publish 接続は呼び出し側契約として定義
- **トレードオフ**: end-to-end の自動配信はラッチ駆動の配線（境界外）が済むまで完成しない。テストは evidence 直接投入で検証する
- **フォローアップ**: 観測対象ゼロ件 DC はラッチからエントリ削除し、削除時に reachable イベントを発行しない（unreachable のまま消えた事実は遷移として扱わない）

### 判断: Trace Field 契約は const 定義の単一ファイルとし、型階層を作らない
- **背景**: 要件 4.3（単一の公開定義）。core は no_std のため tracing 出力自体は持ち込まない
- **検討した代替案**: マーカー型 + trait — 出力責務を持たない契約に型階層は過剰
- **採用したアプローチ**: `topology/cluster_lifecycle_trace_field.rs` に遷移種別名と field 名の `pub const` 群を定義し、std 層 3 ファイルの tracing 出力を契約参照に修正
- **トレードオフ**: const 文字列契約はコンパイル時の使用強制が弱い（レビュー / テストで担保）

## リスクと緩和策

- `ClusterEvent` variant 追加による未知の網羅 match 破壊 — コンパイラの網羅性チェックで全件検出されるため、ビルドを修正完了の判定に使う
- 専用イベント併発による既存購読者への二重通知 — 種別フィルタ前提の購読 API（`event_types` 指定必須）であり、新種別は明示購読しない限り届かない
- ordering 集約時の挙動差（フィルタ条件の取り違え） — 置き換え前後で既存テストを無変更で通すことを完了条件にする

## 参考資料

- `docs/gap-analysis/cluster-gap-analysis.md` — カテゴリ1/2 の parity gap 根拠
- `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/ClusterEvent.scala` — `MemberPreparingForShutdown` / `UnreachableDataCenter` の semantics
- `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/ClusterLogMarker.scala` — trace field 契約の参照元
- `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Member.scala` — `Member.ordering` / `ageOrdering` の semantics
