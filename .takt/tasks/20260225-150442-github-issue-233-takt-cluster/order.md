## GitHub Issue #233: [TAKT] cluster-leader-and-downing (20260224-100008-cls-ldr)

## 元タスク
- slug: 20260224-100008-cls-ldr
- task_dir: .takt/tasks/20260224-100008-cls-ldr
- source: .takt/tasks/20260224-100008-cls-ldr/order.md

## タスク仕様（order.md）

# タスク仕様

## 目的

clusterモジュールにリーダー選出、ダウニング戦略、Gossipプロトコル拡張（Phase 3: medium）を実装し、Pekko Clusterの高度な障害管理と一貫性を追加する。

## 要件

- [ ] `DowningProvider` traitを実装する（ダウニング戦略のプラグインインターフェース）
- [ ] `Cluster::down(address)` メソッドを実装する（明示的なノードダウン指示）
- [ ] リーダー選出メカニズムを実装する（oldest node based leader election）
- [ ] `VectorClock` を実装する（Gossip収束判定用）
- [ ] Gossipプロトコルに `seen` / `SeenChanged` 追跡を追加する
- [ ] `CurrentClusterState` をイベントフィールドで充実させる（leader, roleLeader, unreachable等）
- [ ] `PreparingForShutdown` / `ReadyForShutdown` ステータスを追加する
- [ ] `ReachableMember` / `UnreachableMember` イベントを実装する
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- DowningProviderがプラグインとして差し替え可能
- リーダー選出がMembershipのoldest nodeベースで動作する
- VectorClockがGossip収束判定に使用可能
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/cluster-gap-analysis.md`（カテゴリ3, 4, 5, 7）
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/DowningProvider.scala`
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/VectorClock.scala`
- Pekko参照: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Gossip.scala`


### Labels
takt