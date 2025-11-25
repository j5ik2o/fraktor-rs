# 実装計画

## タスク概要

本計画は `PartitionIdentityLookup` 機能の実装タスクを定義する。TDD 方式で進め、各タスクは 1-3 時間で完了可能な粒度に分割されている。

---

## タスク一覧

- [ ] 1. VirtualActorRegistry の拡張
  - _対応要件: 5, 7, 11_
  - _依存タスク: -_

- [ ] 1.1 VirtualActorRegistry に `remove_activation` メソッドを追加
  - 指定された GrainKey のアクティベーションレコードを削除する機能を実装
  - 対応するキャッシュエントリも同時に削除
  - 存在しないキーに対しては何もせずに正常終了
  - 削除成功時に適切なイベントを生成するか検討
  - ユニットテストで削除前後の状態を検証
  - _対応要件: 7.1, 7.2, 7.3_
  - _依存タスク: -_
  - _完了条件: remove_activation のテストがすべてパスし、既存テストも影響なし_

- [ ] 1.2 VirtualActorRegistry に `drain_cache_events` メソッドを追加
  - 内部の PidCache から蓄積されたイベントを取得する機能を実装
  - PidCacheEvent のベクターを返却
  - 取得後はイベントバッファをクリア
  - ユニットテストでイベント収集と排出を検証
  - _対応要件: 11.2_
  - _依存タスク: 1.1_
  - _完了条件: drain_cache_events のテストがすべてパス_

- [ ] 2. IdentityLookup トレイトの拡張
  - _対応要件: 1, 2, 6, 7, 8, 11_
  - _依存タスク: 1.2_

- [ ] 2.1 IdentityLookup トレイトのシグネチャを `&mut self` に変更
  - 既存の `setup_member` と `setup_client` のシグネチャを `&mut self` に変更
  - デフォルト実装は空の Ok(()) を返す
  - コンパイルエラーを確認し、影響範囲を把握
  - _対応要件: 1.1, 1.4_
  - _依存タスク: 1.2_
  - _完了条件: トレイト定義の変更が完了し、コンパイルエラーがリスト化_

- [ ] 2.2 IdentityLookup トレイトに PID 解決系メソッドを追加
  - `get(&mut self, key: &GrainKey, now: u64) -> Option<String>` を追加
  - `remove_pid(&mut self, key: &GrainKey)` を追加
  - デフォルト実装は get が None、remove_pid が何もしない
  - _対応要件: 2.1, 2.2, 2.3, 2.4, 2.5, 7.1, 7.2, 7.3_
  - _依存タスク: 2.1_
  - _完了条件: メソッド追加が完了_

- [ ] 2.3 IdentityLookup トレイトにトポロジ管理系メソッドを追加
  - `update_topology(&mut self, authorities: Vec<String>)` を追加
  - `on_member_left(&mut self, authority: &str)` を追加
  - デフォルト実装は何もしない
  - _対応要件: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_
  - _依存タスク: 2.2_
  - _完了条件: メソッド追加が完了_

- [ ] 2.4 IdentityLookup トレイトにパッシベーション・イベント系メソッドを追加
  - `passivate_idle(&mut self, now: u64, idle_ttl: u64)` を追加
  - `drain_events(&mut self) -> Vec<VirtualActorEvent>` を追加
  - `drain_cache_events(&mut self) -> Vec<PidCacheEvent>` を追加
  - デフォルト実装は passivate_idle が何もせず、drain 系は空ベクターを返却
  - _対応要件: 8.1, 8.2, 8.3, 11.1, 11.2, 11.3, 11.4, 11.5_
  - _依存タスク: 2.3_
  - _完了条件: メソッド追加が完了_

- [ ] 3. NoopIdentityLookup の更新
  - _対応要件: 1_
  - _依存タスク: 2.4_

- [ ] 3.1 NoopIdentityLookup を新しいトレイトシグネチャに適合
  - `setup_member` と `setup_client` を `&mut self` に変更
  - 新規追加されたメソッドはトレイトのデフォルト実装を継承
  - 既存のテストが引き続きパスすることを確認
  - _対応要件: 1.1_
  - _依存タスク: 2.4_
  - _完了条件: NoopIdentityLookup が新しいトレイトに適合し、テストパス_

- [ ] 4. LookupError の作成
  - _対応要件: 2_
  - _依存タスク: -_

- [ ] 4.1 LookupError 型を新規作成
  - NoAuthority、ActivationFailed、Timeout のバリアントを定義
  - ActivationFailed は GrainKey 情報を保持
  - Debug、Clone、PartialEq、Eq を derive
  - no_std 互換（alloc::string::String を使用）
  - _対応要件: 2.4_
  - _依存タスク: -_
  - _完了条件: 型定義が完了し、コンパイル成功_

- [ ] 5. PartitionIdentityLookupConfig の作成
  - _対応要件: 10_
  - _依存タスク: -_

- [ ] 5.1 設定構造体を新規作成
  - cache_capacity、pid_ttl_secs、idle_ttl_secs フィールドを定義
  - コンストラクタと各フィールドのゲッターを実装
  - Default トレイトを実装（キャッシュ容量 1024、PID TTL 300秒、アイドル TTL 3600秒）
  - Debug、Clone を derive
  - ユニットテストでデフォルト値とカスタム値の生成を検証
  - _対応要件: 10.1, 10.2, 10.3, 10.4, 10.5_
  - _依存タスク: -_
  - _完了条件: 設定構造体のテストがすべてパス_

- [ ] 6. PartitionIdentityLookup の基本実装
  - _対応要件: 1, 2, 3, 4, 5_
  - _依存タスク: 3.1, 4.1, 5.1_

- [ ] 6.1 構造体の定義と基本コンストラクタを実装
  - VirtualActorRegistry、authorities、member_kinds、client_kinds、config フィールドを定義
  - new(config) と with_defaults() コンストラクタを実装
  - authorities() と config() ゲッターを実装
  - Send + Sync が自動導出されることを確認
  - _対応要件: 1.1, 1.4, 1.5, 10.6_
  - _依存タスク: 5.1_
  - _完了条件: 構造体が定義され、コンパイル成功_

- [ ] 6.2 setup_member と setup_client を実装
  - 提供された ActivatedKind リストを内部フィールドに保存
  - IdentityLookup トレイトを実装
  - ユニットテストでセットアップ後の状態を検証
  - _対応要件: 1.2, 1.3_
  - _依存タスク: 6.1_
  - _完了条件: setup 系メソッドのテストがパス_

- [ ] 6.3 get メソッドを実装（キャッシュヒット時）
  - VirtualActorRegistry の cached_pid を使用してキャッシュを確認
  - キャッシュヒット時は即座に PID を返却
  - イベント生成なし（高速パス）
  - ユニットテストでキャッシュヒットのシナリオを検証
  - _対応要件: 2.1, 2.2_
  - _依存タスク: 6.2_
  - _完了条件: キャッシュヒット時のテストがパス_

- [ ] 6.4 get メソッドを実装（キャッシュミス時）
  - キャッシュミス時は ensure_activation を呼び出し
  - authorities リストを渡してオーナーノードを選定
  - アクティベーション成功時は PID を返却
  - アクティベーション失敗時は None を返却
  - ユニットテストでキャッシュミス→アクティベーションのフローを検証
  - _対応要件: 2.1, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3_
  - _依存タスク: 6.3_
  - _完了条件: キャッシュミス時のテストがパス_

- [ ] 6.5 remove_pid メソッドを実装
  - VirtualActorRegistry の remove_activation を呼び出し
  - 存在しないキーに対してもエラーなく完了
  - ユニットテストで削除前後の状態を検証
  - _対応要件: 7.1, 7.2, 7.3_
  - _依存タスク: 6.4_
  - _完了条件: remove_pid のテストがパス_

- [ ] 7. トポロジ管理機能の実装
  - _対応要件: 3, 6_
  - _依存タスク: 6.5_

- [ ] 7.1 update_topology メソッドを実装
  - 新しい authorities リストを内部フィールドに保存
  - invalidate_absent_authorities を呼び出して不在 authority のエントリを無効化
  - ユニットテストでトポロジ更新と無効化を検証
  - _対応要件: 6.3, 6.4, 3.4_
  - _依存タスク: 6.5_
  - _完了条件: update_topology のテストがパス_

- [ ] 7.2 on_member_left メソッドを実装
  - VirtualActorRegistry の invalidate_authority を呼び出し
  - 該当 authority のキャッシュとアクティベーションを無効化
  - ユニットテストでメンバー離脱時の無効化を検証
  - _対応要件: 6.1, 6.2, 6.5_
  - _依存タスク: 7.1_
  - _完了条件: on_member_left のテストがパス_

- [ ] 8. パッシベーションとイベント機能の実装
  - _対応要件: 8, 11_
  - _依存タスク: 7.2_

- [ ] 8.1 passivate_idle メソッドを実装
  - VirtualActorRegistry の passivate_idle を呼び出し
  - 指定 TTL を超えたアクティベーションを削除
  - 対応するキャッシュエントリも削除
  - Passivated イベントを生成
  - ユニットテストでアイドルパッシベーションを検証
  - _対応要件: 8.1, 8.2, 8.3_
  - _依存タスク: 7.2_
  - _完了条件: passivate_idle のテストがパス_

- [ ] 8.2 drain_events と drain_cache_events を実装
  - VirtualActorRegistry から対応するメソッドを呼び出し
  - イベントベクターを返却
  - ユニットテストでイベント収集を検証
  - _対応要件: 11.1, 11.2, 11.3, 11.4, 11.5_
  - _依存タスク: 8.1_
  - _完了条件: イベント系メソッドのテストがパス_

- [ ] 9. ClusterCore の統合
  - _対応要件: 9_
  - _依存タスク: 8.2_

- [ ] 9.1 ClusterCore の identity_lookup フィールドを ToolboxMutex に変更
  - `ArcShared<dyn IdentityLookup>` を `ToolboxMutex<Box<dyn IdentityLookup>>` に変更
  - 関連するフィールドの型も更新
  - コンパイルエラーを確認し、影響範囲を把握
  - _対応要件: 9.1_
  - _依存タスク: 8.2_
  - _完了条件: フィールド型の変更が完了_

- [ ] 9.2 ClusterCore のメソッドを新しい IdentityLookup に対応
  - setup_member_kinds から identity_lookup.setup_member を呼び出し
  - setup_client_kinds から identity_lookup.setup_client を呼び出し
  - get_pid メソッドを追加し、identity_lookup.get を呼び出し
  - ロック取得後に &mut self を渡す
  - _対応要件: 9.2, 9.3_
  - _依存タスク: 9.1_
  - _完了条件: メソッド呼び出しの修正が完了_

- [ ] 9.3 ClusterCore のトポロジ更新から IdentityLookup へ伝播
  - トポロジ更新時に identity_lookup.update_topology を呼び出し
  - メンバー離脱時に identity_lookup.on_member_left を呼び出し
  - ユニットテストでトポロジ変更の伝播を検証
  - _対応要件: 9.4_
  - _依存タスク: 9.2_
  - _完了条件: トポロジ伝播のテストがパス_

- [ ] 10. モジュールエクスポートとコンパイル確認
  - _対応要件: 1, 10_
  - _依存タスク: 9.3_

- [ ] 10.1 core.rs にモジュールエクスポートを追加
  - partition_identity_lookup モジュールを追加
  - partition_identity_lookup_config モジュールを追加
  - lookup_error モジュールを追加
  - pub use で公開 API を再エクスポート
  - _対応要件: 1.1, 10.1_
  - _依存タスク: 9.3_
  - _完了条件: エクスポート追加が完了_

- [ ] 10.2 no_std ビルドの確認
  - `scripts/ci-check.sh no-std` を実行
  - alloc クレートのみ使用していることを確認
  - embedded ターゲットでのコンパイルも確認
  - _対応要件: 1.5_
  - _依存タスク: 10.1_
  - _完了条件: no_std ビルドが成功_

- [ ] 11. 統合テスト
  - _対応要件: 全要件の統合検証_
  - _依存タスク: 10.2_

- [ ] 11.1 PartitionIdentityLookup の統合テストを作成
  - 完全な PID 解決フロー（キャッシュミス→アクティベーション→キャッシュヒット）を検証
  - トポロジ変更とキャッシュ無効化の連携を検証
  - アイドルパッシベーションとイベント生成を検証
  - _対応要件: 2, 6, 8, 11_
  - _依存タスク: 10.2_
  - _完了条件: 統合テストがすべてパス_

- [ ] 11.2 ClusterCore 経由での PartitionIdentityLookup 使用テストを作成
  - ClusterExtensionInstaller で PartitionIdentityLookup を設定
  - setup_member_kinds 経由でのセットアップを検証
  - get_pid 経由での PID 解決を検証
  - トポロジ更新の自動伝播を検証
  - _対応要件: 9_
  - _依存タスク: 11.1_
  - _完了条件: ClusterCore 統合テストがすべてパス_

- [ ] 12. 最終検証
  - _対応要件: 全要件_
  - _依存タスク: 11.2_

- [ ] 12.1 CI チェックの完全実行
  - `scripts/ci-check.sh all` を実行
  - すべてのテストがパスすることを確認
  - clippy 警告がないことを確認
  - ドキュメント生成が成功することを確認
  - _対応要件: 全要件_
  - _依存タスク: 11.2_
  - _完了条件: CI チェックがすべて成功_

---

## 要件カバレッジマトリックス

| 要件ID | 要件概要 | 対応タスク |
|--------|---------|-----------|
| 1 | IdentityLookup トレイト実装 | 2.1, 2.2, 2.3, 2.4, 3.1, 6.1, 6.2, 10.1 |
| 2 | Grain PID 解決 | 2.2, 4.1, 6.3, 6.4, 6.5 |
| 3 | オーナーノード選定 | 6.4, 7.1 |
| 4 | PID キャッシュ統合 | 6.3, 6.4 |
| 5 | VirtualActorRegistry 統合 | 1.1, 1.2, 6.3, 6.4 |
| 6 | トポロジ変更対応 | 2.3, 7.1, 7.2 |
| 7 | PID 削除 | 1.1, 2.2, 6.5 |
| 8 | アイドルパッシベーション | 2.4, 8.1 |
| 9 | ClusterCore 統合 | 9.1, 9.2, 9.3, 11.2 |
| 10 | 設定提供 | 5.1, 6.1, 10.1 |
| 11 | イベント通知 | 1.2, 2.4, 8.2 |

---

## 実装順序の根拠

1. **VirtualActorRegistry 拡張を最初に**: `PartitionIdentityLookup` が依存する `remove_activation()` と `drain_cache_events()` メソッドが必要
2. **IdentityLookup トレイト拡張**: 新しいシグネチャとメソッドを定義してから実装クラスを作成
3. **NoopIdentityLookup 更新**: 既存実装を壊さないよう、トレイト変更後すぐに適合
4. **LookupError と Config を並行**: 独立した型なので早期に作成可能
5. **PartitionIdentityLookup 本体**: 依存関係が整った後に段階的に実装
6. **ClusterCore 統合**: PartitionIdentityLookup が完成してから統合
7. **最終検証**: すべての実装完了後に CI チェック
