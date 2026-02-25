## GitHub Issue #232: [TAKT] cluster-member-model (20260224-100007-cls-mbr)

## 元タスク
- slug: 20260224-100007-cls-mbr
- task_dir: .takt/tasks/20260224-100007-cls-mbr
- source: .takt/tasks/20260224-100007-cls-mbr/order.md

## タスク仕様（order.md）

# タスク仕様

## 目的

clusterモジュールにメンバーモデルの拡張フィールドとステータス（Phase 1-2: trivial〜easy）を実装し、Pekko Clusterのメンバー管理との互換性を向上させる。

## 要件

- [ ] `NodeRecord`（Member相当）に `app_version` フィールドを追加する
- [ ] `NodeRecord` に `roles` フィールドを追加する
- [ ] `NodeStatus` に `Exiting` ステータスを追加する
- [ ] `is_older_than` メソッドを実装する（メンバー間の年齢比較）
- [ ] `register_on_member_up` / `register_on_member_removed` コールバック登録を実装する
- [ ] `JoinConfigCompatChecker` traitを実装する（ノード参加時の設定互換性検証）
- [ ] `ClusterSettings` にロール設定を追加する
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- NodeRecordがPekkoのMemberクラスと同等のフィールドを持つ
- メンバーステータスにExitingが追加され、状態遷移が正しく動作する
- コールバック登録APIが利用可能
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/cluster-gap-analysis.md`（カテゴリ1, 2, 6）
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Member.scala`
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/MemberStatus.scala`
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/JoinConfigCompatChecker.scala`
