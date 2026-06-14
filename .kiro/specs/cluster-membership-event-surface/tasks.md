# 実装計画

- [x] 1. 基盤: イベント語彙とトレースフィールド契約
- [x] 1.1 cluster イベント語彙に shutdown 進行と DC 到達性の 4 種別を追加する
  - shutdown 準備開始・完了（member 識別 + 観測時刻）と data center の unreachable / reachable（data center 識別 + 観測時刻）のイベントを既存 variant のフィールド規約に合わせて追加する
  - イベント種別フィルタに対応 4 種別を追加し、種別照合の網羅 match を更新する
  - 既存のイベント種別は削除・変更しない
  - 完了条件: ワークスペースがコンパイルでき、新 4 種別の種別照合テストが通る
  - _Requirements:_ 2.1, 2.2, 2.3, 2.4, 3.3, 3.5, 5.4
  - _Boundary:_ ClusterEvent / ClusterEventType 拡張
  - _Depends:_ none

- [x] 1.2 (P) cluster lifecycle のトレースフィールド契約を単一定義として追加する
  - 遷移種別（join / up / leave / removal / shutdown 進行 / DC 到達性変化）ごとに一意な値を持つ定数群と、member 識別・data center を表すフィールド名定数を 1 ファイルで定義する
  - topology の wiring から公開する
  - 完了条件: 遷移種別値の一意性テストが通り、契約が単一ファイルから公開される
  - _Requirements:_ 4.1, 4.2, 4.3
  - _Boundary:_ cluster_lifecycle_trace_field
  - _Depends:_ none

- [x] 2. コア: 順序契約・DC 到達性ラッチ・イベント併発
- [x] 2.1 (P) Member Ordering の公開契約を実装する
  - 既存のペア比較（join 古さ + authority tie-break）を正本として、全順序比較・age 順整列・最古メンバー特定の pure な公開関数群を membership に追加する
  - フィルタ（active / leader-eligible）は持ち込まず順序のみを所有する
  - membership の wiring に新モジュール宣言と pub use を追加し、公開 API として到達可能にする
  - 入力順をシャッフルしても同一の並びになる決定性、tie-break の一意解決、空集合での「最古メンバーなし」をテストで検証する
  - 完了条件: 順序契約の単体テストが通り、membership の公開 API として到達可能になる
  - _Requirements:_ 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 5.3
  - _Boundary:_ member_ordering
  - _Depends:_ none

- [x] 2.2 (P) DC 到達性ラッチを実装する
  - cross-DC heartbeat の観測対象変更と evidence を入力に、DC 単位の unreachable ラッチと復帰判定を行う値型の状態機械と遷移出力 enum を membership に追加する
  - 自 DC の evidence は入力段階で無視し、観測対象が空になった DC はエントリ削除（遷移出力なし）とする
  - down 操作・membership 変更は行わず、出力は遷移 enum のみとする
  - membership の wiring にラッチと遷移 enum のモジュール宣言と pub use を追加する
  - 完了条件: evidence 直接投入のテストで「全観測対象 unreachable で 1 回だけ遷移出力」「復帰で 1 回出力」「自 DC 無視」「ゼロ件削除」の遷移列が検証される
  - _Requirements:_ 3.1, 3.2, 3.4, 3.6
  - _Boundary:_ DataCenterReachabilityTable
  - _Depends:_ none

- [x] 2.3 (P) shutdown 進行イベントの併発を membership coordinator に追加する
  - status 遷移の発行集約点で、遷移先が shutdown 準備開始・完了のとき、従来の status 変更イベントの後に専用イベントを併発する
  - 状態遷移の規則自体は変更しない
  - local 起点の遷移と gossip 経由の遷移の両方で、併発順序（status 変更 → 専用イベント）をテストで検証する
  - 完了条件: 併発を検証する coordinator テストが通り、既存の coordinator テストが無変更で通る
  - _Requirements:_ 2.1, 2.2, 2.5, 5.1, 5.4
  - _Boundary:_ MembershipCoordinator 変更
  - _Depends:_ 1.1

- [x] 3. 統合: 重複実装の集約と std 準拠化
- [x] 3.1 (P) coordinator の oldest / role leader 算出を順序契約へ委譲する
  - leader 算出のプライベートな oldest 判定と role leader の比較部分を、順序契約の参照に置き換える（leader-eligible フィルタは coordinator 側に残す）
  - 2.3 の併発実装が完了した後に着手する（同一ファイルへの変更競合を避ける）
  - 完了条件: 既存の leader / role leader 算出テストが無変更で通る
  - _Requirements:_ 1.5
  - _Boundary:_ MembershipCoordinator 変更, member_ordering（統合タスク）
  - _Depends:_ 2.1, 2.3

- [x] 3.2 (P) SBR KeepOldest の oldest 判定を順序契約へ委譲する
  - SBR 内のプライベートな oldest 判定を削除し、active フィルタ済み集合への順序契約適用に置き換える（判定規則・trace 形状は変更しない）
  - 完了条件: 既存の split brain resolver テストが無変更で通る
  - _Requirements:_ 1.5
  - _Boundary:_ SplitBrainResolver 変更, member_ordering（統合タスク）
  - _Depends:_ 2.1

- [x] 3.3 (P) std 層の tracing 出力をトレースフィールド契約に準拠させる
  - gossip transport / gossiper / AWS ECS provider の cluster lifecycle 関連 tracing 出力を契約の定数参照に置き換える（出力タイミング・ログレベルは変更しない）
  - 完了条件: 対象 3 ファイルの lifecycle 関連出力がすべて契約定数を参照し、アドホックなフィールド名が残らない
  - _Requirements:_ 4.4
  - _Boundary:_ std tracing 準拠化
  - _Depends:_ 1.2

- [x] 4. 検証
- [x] 4.1 購読フィルタと配信の統合検証を追加する
  - 新イベント種別だけを指定した購読者が shutdown 進行イベントのみを受信することを cluster の購読 API 経由で検証する
  - DC ラッチの遷移出力（`DataCenterReachabilityTransition`）から `UnreachableDataCenter` / `ReachableDataCenter` の `ClusterEvent` への変換ロジックを実装し、`data_center` 識別子が正しく引き継がれることをテストで検証する
  - cluster の公開 API に full shutdown を開始する command が追加されていないことを確認する
  - 完了条件: 購読フィルタの統合テストと変換テストが通る
  - _Requirements:_ 2.4, 3.1, 3.2, 5.2
  - _Boundary:_ ClusterApi 購読統合（extension + topology + membership の統合タスク）
  - _Depends:_ 2.2, 2.3

- [x] 4.2 既存挙動の回帰確認と対象範囲のチェックを実行する
  - cluster-core-kernel / cluster-adaptor-std の unit-test、clippy、対象 dylint、no-std チェックを実行する
  - 既存イベント種別が削除されていないこと、既存テストが無変更で通ることを確認する
  - 完了条件: 対象範囲のチェックがすべて成功する
  - _Requirements:_ 5.1, 5.4
  - _Boundary:_ クレート横断検証（統合タスク）
  - _Depends:_ 3.1, 3.2, 3.3

## Implementation Notes

- 1.1: ClusterEventType の matches は const fn の網羅 match。variant 追加時は cluster_event.rs / cluster_event_type.rs / 同 matches の 3 点セットで更新する
- 1.2: trace field 契約に FIELD_TRANSITION（"cluster.lifecycle.transition"）が設計外で追加されている（レビュー承認済み）。3.3 の std 準拠化ではこの定数を遷移種別の出力 key として使うこと
- 2.3: shutdown 系遷移の実発行点は emit_status_change ではなく register_membership_change（gossip delta パス）だった。design.md は訂正済み。3.1 で coordinator を触る際は両方の発行点に注意
- 3.3: std 層の既存 tracing 出力（5 件）は全て I/O 系エラーで cluster lifecycle 遷移の記録は現存しない。要件 4.4 は条件付き要件として真空的に成立（コード変更なしで完了、レビュー承認済み）。→ 後続のフォローアップで cluster-adaptor-std に `ClusterLifecycleLogSubscriber` を実装し、契約定数の実消費者が存在する状態になった（要件 4.4 が実質的に行使される形で成立）。以降 std 層に lifecycle 出力を追加する際も cluster_lifecycle_trace_field の契約定数を必ず参照すること
- 4.2: `cluster_core_test.rs:1010,1054` の pre-existing clippy エラー（comparison to empty slice）2 件は検証完了後にボーイスカウト修正済み（`.is_empty()` 化）。`./scripts/ci-check.sh clippy` の cluster クレート除外（2026-03-15 導入、当時の nightly-2025-12-01 と postcard 1.1.3 の非互換が理由）は現行 nightly-2026-06-05 で解消を確認し撤去済み。cluster クレートも CI の clippy 対象に復帰
