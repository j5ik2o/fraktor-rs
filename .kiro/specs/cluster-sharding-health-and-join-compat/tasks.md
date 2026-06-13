# 実装計画

- [x] 1. コア: readiness 判定契約と互換キー目録を整備する
- [x] 1.1 (P) readiness 判定の型と判定規則を実装する
  - 入力（自ノードの membership 状態・placement 調整状態・登録済み kind）の写しから、固定仕様（稼働 = Up / WeaklyUp、解決可能 = Member / Client、期待 kind の包含）で ready / not ready と原因種別を導出する純粋な判定を定義する
  - 欠けた条件はすべて原因として列挙し、期待 kind が空のときは kind 条件を課さない
  - 3条件成立で Ready、各条件欠如で対応する原因種別、複数欠如で複数原因、同一入力 → 同一結果を検証する sibling テストが green になる
  - _Requirements:_ 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 4.3, 4.4
  - _Boundary:_ GrainReadinessSnapshot / GrainReadiness / GrainUnreadyReason
  - _Depends:_ none
- [x] 1.2 (P) grain / placement 領域の除外キーを目録へ登録する
  - identity lookup の実装選択とローカルチューニング値を表す除外キー2件を、既存 excluded key の文体を踏襲した理由文とともに登録し、将来拡張の選定基準（ノード間で一致しないと配送・配置の正しさが壊れる値のみ）を目録の rustdoc に記載する
  - 除外キー2件が excluded_keys() に理由付きで含まれ、required / conditional の評価対象に含まれないことを検証する目録テストが green になり、既存の join 互換テストが無変更で green になる
  - _Requirements:_ 3.1, 3.2, 3.3, 4.2
  - _Boundary:_ ClusterCompatibilityKeyCatalog
  - _Depends:_ none
- [x] 1.3 (P) placement 状態の読み取りクエリを IdentityLookup port に追加する
  - port にデフォルト実装付きの placement 状態クエリ（既定で解決不能を返す）を追加し、placement coordinator を内包する実装だけが override して実状態を返す。既存の実装型は無変更で従来挙動を維持する
  - override が coordinator の状態を返すこと、デフォルト実装が解決不能を返すこと（テストローカル実装で確認）を検証する sibling テストが green になる
  - _Requirements:_ 1.1
  - _Boundary:_ IdentityLookup / PartitionIdentityLookup
  - _Depends:_ none

- [x] 2. 統合: 写し構築の公開アクセサを追加する
  - core の既存状態の読み取り（自ノード record の status — 現状は在籍メンバーを Up と報告する忠実度をそのまま使う、port 経由の placement 状態、登録済み kind 名）だけで判定入力の写しを構築する集約クエリを core に追加し、extension に公開アクセサを追加する明示的な統合タスク（既存の状態遷移・挙動には触れない）
  - 実 system fixture で、起動前のアクセサ呼び出しが not ready を導く写し（原因が観測可能）を返し、member 起動 + kind 登録後は ready を導く写しを返すことを検証する sibling テストが green になる
  - _Requirements:_ 2.1, 2.2, 2.3, 4.1
  - _Boundary:_ ClusterCore / ClusterExtension（統合）
  - _Depends:_ 1.1, 1.3

- [x] 3. 検証: 非回帰と範囲限定を確認する
  - 既存テスト（join 互換・membership・placement・grain）が無変更で green になることを確認する（4.1, 4.2）
  - 判定3型が alloc / core のみに依存し、no_std チェックが通過することを確認する（4.3）
  - ホスト層（adaptor-std）に差分がないこと、placement / activation の挙動・既存 join 評価結果に変更がないことを差分で確認する（4.1, 4.2）
  - 対象 crate の targeted check（cargo test -p、clippy / dylint）が exit 0 で通過する
  - _Requirements:_ 4.1, 4.2, 4.3
  - _Boundary:_ 全体検証
  - _Depends:_ 2

## Implementation Notes

- 自ノード status は `ClusterCore::current_cluster_state_snapshot()` 経由で取得。現状は在籍メンバーを `NodeStatus::Up` と報告する忠実度のため、判定は「在籍 = 稼働」を反映する。忠実度向上時は判定が自動追従する（設計どおり）。
- `cluster-core-kernel` は `no-std` host-core クロスチェックの対象外。判定3型の no_std 完結は crate の `#![deny(cfg_std_forbid)]` + `cfg-std-forbid-lint`（編集ごとの hook で機械的に強制）で担保した。
- 編集後 hook が `ci-check.sh ai fmt dylint clippy` を全 workspace で実行するため、clippy `missing_const_for_fn` / `unused_imports` / `dead_code` が中間状態で検出される。型と配線を1コミット単位で整合させること。
