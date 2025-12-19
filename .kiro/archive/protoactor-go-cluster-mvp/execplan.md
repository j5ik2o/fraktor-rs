# protoactor-go-cluster-mvp ExecPlan

この ExecPlan は .agent/PLANS.md に従って維持する。作業中は本書だけで新人が実装を再現できるよう、すべての前提と手順をここに記す。

## Purpose / Big Picture

protoactor-go 互換クラスタの MVP を fraktor-rs に追加し、メンバーシップ管理・Gossip 収束・Virtual Actor ルーティング・RPC を Rust/no_std で再現する。まずはタスク1.1（参加/離脱/到達不能とハンドシェイク）を完了させ、メンバーシップテーブルの状態遷移がローカルテストで確認できる状態を作る。

## Progress

- [x] (2025-11-21) 要件・設計・ステアリングを読込み、タスク1.1を初期着手範囲として選定
- [x] (2025-11-21) メンバーシップテーブルのユニットテストを追加し、RED を確認
- [x] (2025-11-21) 最小実装でテストをGREEN化し、リファクタ完了（`cargo test -p fraktor-cluster-rs`）
- [x] (2025-11-21) タスク1.1のチェックボックスを更新し、ExecPlanを反映
- [x] (2025-11-21) GossipEngine のREDテストを追加（Diffusing→Confirmed, conflict, missing range）
- [x] (2025-11-21) GossipEngine を実装してGREEN化、タスク1.2を完了
- [x] (2025-11-21) IdentityTable/ResolveResult/ResolveError/IdentityEvent を追加し、PID解決テスト（Ready/Unreachable/Quarantine/InvalidFormat/最新バージョン維持）をRED→GREEN化、タスク2.1を完了

## Surprises & Discoveries

- 現時点なし。新規発見があればエビデンス（テスト出力等）とともに追記する。

## Decision Log

- Decision: クラスタ機能を新規クレート`modules/cluster`（crate名: `fraktor-cluster-rs` 仮）として追加し、既存クレートと同様に `core`/`std` 二層構成・2018モジュール・1ファイル1型方針を採用する。
  Rationale: ステアリングの依存方向（utils→actor→remote）を崩さず、クラスタ領域を分離して拡張性を確保するため。
  Date/Author: 2025-11-21 / Codex

## Outcomes & Retrospective

完了後に、達成内容・残課題・学びをここに記す。

## Context and Orientation

- 現状ワークスペースには `modules/utils`, `modules/actor`, `modules/remote` のみで、クラスタ用クレートは存在しない。
- ステアリングで要求される構造: no_std 前提、`mod.rs` 不使用、1ファイル1型、rustdocのみ英語、その他コメント/Markdownは日本語。
- タスク1.1の受け入れ条件: Joining/Leaving/Removed/Unreachable の状態機械、ハートビート欠落で Unreachable へ遷移、authority 衝突の拒否と EventStream 通知、ハンドシェイクで最新テーブル配布。
- 依存順序: 1.1 が基盤となり 1.2 以降が連鎖するため、まずメンバーシップテーブルとデルタ/ハンドシェイクの最小部品を core に置く。

## Plan of Work

1. 新規クレート `modules/cluster` を追加し、`Cargo.toml` と `src/lib.rs`/`core.rs` に deny lint と no_std 設定を揃える。crate 名は `fraktor-cluster-rs` とし、workspace へ登録する。まだ std 実装は空でも良い。
2. `modules/cluster/src/core` 配下に以下を追加する（1ファイル1型原則）。
   - `node_status.rs`: 公開 enum `NodeStatus`（Joining/Up/Leaving/Removed/Unreachable）。
   - `node_record.rs`: 公開構造体 `NodeRecord`（node_id, authority, status, version）。
   - `membership_version.rs`: 公開新型 `MembershipVersion`（単調増加カウンタ）。
   - `membership_delta.rs`: 公開構造体 `MembershipDelta`（バージョン帯とエントリ差分）。
   - `membership_table.rs`: 公開構造体 `MembershipTable` と `apply_delta`/`handshake_snapshot`/`mark_heartbeat_miss` 等のメソッド。
3. `membership_table/tests.rs` にタスク1.1のシナリオを RED テストとして追加する。
   - 正常参加: 衝突なしで Joining→Up、全メンバーに通知されるスナップショットが最新バージョンを含む。
   - authority 衝突: 同一 authority 異 node_id で拒否し EventStream 用イベントが記録される（イベントは列挙値で代用）。
   - 離脱: Leaving→Removed に遷移しテーブル反映。
   - ハートビート欠落: miss カウント閾値到達で Unreachable へ遷移。
4. テストを RED で確認後、最小実装を追加して GREEN 化。重複をリファクタ（内部関数抽出、バージョン管理の一貫性）。
5. `tasks.md` の 1.1 を [x] に更新し、必要なら ExecPlan Progress を更新。ローカルでは `cargo test -p fraktor-cluster-rs` を実行し、最後に影響範囲のテストへ広げる。

## Concrete Steps

- 作業ディレクトリ: `/Users/j5ik2o/Sources/fraktor-rs`
- コマンド例（進行に応じて更新）:
  - クレート追加後: `cargo test -p fraktor-cluster-rs`
  - 部分テスト: `cargo test -p fraktor-cluster-rs membership_table`
  - 影響範囲確認: `cargo test`

## Validation and Acceptance

- テスト観点: `membership_table/tests.rs` に追加した4シナリオが RED→GREEN となること。
- 受け入れ: `cargo test -p fraktor-cluster-rs` がパスし、`tasks.md` の 1.1 チェックボックスが [x] となっている。
- 追加確認: 新規クレートが workspace に組み込まれ `cargo metadata` で認識される。

## Idempotence and Recovery

- 何度でもテスト実行可能。バージョンカウンタはテスト内で初期化するため副作用なし。
- 新規ファイル追加のみで既存コードを壊さない。生成物を削除する場合は `modules/cluster` ディレクトリを丸ごと削除し、`Cargo.toml` の該当差分を戻せば元に戻る。

## Artifacts and Notes

- 現時点なし。重要なログやテスト出力が出たらここに抜粋を残す。

## Interfaces and Dependencies

- 依存クレート: `alloc` のみで開始し、no_std 前提。後続タスクで `fraktor-utils-rs` 等へ依存を広げる。
- 公開インターフェイス:
  - `NodeStatus`: ノード状態列挙。
  - `NodeRecord`: ノード ID/authority/状態/バージョン。
  - `MembershipDelta`: 参加/離脱/ステータス変更の差分集合。
  - `MembershipTable`: 上記のテーブル管理とハンドシェイクスナップショット生成を担うコア型。

---
変更履歴:
- 2025-11-21: 初版作成（Codex）。
