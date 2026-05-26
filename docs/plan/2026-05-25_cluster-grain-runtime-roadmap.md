# cluster Grain runtime roadmap

## 決定

`cluster-*` は Apache Pekko Cluster / Cluster Sharding 互換クラスタではなく、Proto.Actor-Go 型の Virtual Actor / Grain runtime として位置づける。

Pekko は parity target ではない。大規模運用で必要になる membership、reachability、downing、placement、rebalance、remembered entities などの失敗ケースと設計論点を確認する参照実装として使う。

## 背景

現在の `cluster-core` は、`GrainRef`、`GrainKey`、`VirtualActorRegistry`、`PartitionIdentityLookup`、`PlacementCoordinatorCore` を中心に、ID から actor を解決して呼び出す Virtual Actor model をすでに持っている。この実装資産と利用者価値を主語にする方が、Pekko の広い cluster API parity を追うよりも焦点が明確になる。

Pekko / Akka Cluster Sharding は大規模運用で成熟した semantics を持つ。一方で、ShardCoordinator や Cluster Singleton を中核にすると、ローリングアップデート時に coordinator の配置と移動を強く意識する必要がある。fraktor-rs ではそのモデルを中核に戻さず、Grain runtime が必要とする運用 contract を段階的に固める。

## 採用方針

採用する主軸:

- Virtual Actor / Grain
- Identity lookup
- Placement resolution
- Activation / passivation
- Membership topology update
- Cluster provider boundary
- Failure detector / downing の最小 contract

Pekko から参照する論点:

- Reachability の表現と failure observation
- Split Brain Resolver / downing strategy の判断モデル
- Shard rebalance の失敗ケース
- Remembered entities が解いている復元要件
- Distributed PubSub / Distributed Data が抱える registry replication の論点

直近では主軸にしないもの:

- Pekko typed Cluster API parity
- ClusterSingleton / ShardCoordinator parity
- ClusterClient / Receptionist parity
- DistributedData / CRDT 全面実装
- Sharding delivery controller parity
- Pekko public API の全面移植

## Pros / Cons

### Grain runtime 主軸

Pros:

- 既存の `cluster-core` 実装資産と一致する。
- API の主語を「ID から actor を解決して呼ぶ」に絞れる。
- ShardCoordinator / Cluster Singleton を中核にしないため、ローリングアップデート時の coordinator 配置問題を避けやすい。
- `no_std` core と `std` adaptor の分離と相性がよい。
- Rust runtime としての差別化になる。

Cons:

- 大規模クラスタで必要な rebalance、remembered entities、reachability、SBR は自前で contract を詰める必要がある。
- Proto.Actor-Go 由来の運用 semantics は Pekko / Akka ほど蓄積が多くない。
- Placement / Identity の責務を曖昧にすると、hidden singleton を作ってしまうリスクがある。

### Pekko Cluster 主軸

Pros:

- Membership、reachability、SBR、Cluster Sharding、Distributed Data など、大規模クラスタの論点が揃っている。
- Akka / Pekko 経験者には概念が伝わりやすい。
- gap analysis と実装タスクの対応が作りやすい。

Cons:

- ShardCoordinator、Cluster Singleton、Distributed Data などの実装とテストが重い。
- coordinator 配置とローリングアップデート影響を設計の中心に戻してしまう。
- 既存の Grain API と二重モデルになりやすい。
- Pekko parity を掲げると分母が大きくなり、戦略がぼやける。

## 直近の成功条件

短期の成功条件は、大規模 Pekko parity ではなく、小中規模から安全に伸ばせる Grain runtime の運用 contract を固定すること。

- Grain identity resolution が安定している。
- node join / leave / down に対して placement cache が破綻しない。
- rolling update 時に activation / routing がどう変わるか説明できる。
- failure detector と downing の最小 contract がある。
- `cluster-adaptor-std` の local / static / AWS ECS provider の動作境界が明確である。

## Task slices

進捗の読み方:

- `~~取消線~~` は OpenSpec archive / current code / tests で完了確認できる項目。
- 取消線なしは未完了または別 change として継続中の項目。

### ~~1. Documentation alignment~~

- ~~`cluster-gap-analysis.md` を Pekko comparison として位置づけ直す。~~
- ~~README / docs で `cluster-*` の主語を Grain runtime として説明する。~~
- ~~Pekko parity ではなく参照実装としての扱いを明記する。~~

### 2. Operational contract tests

- ~~identity lookup の成功 / no authority を contract test として固定する。~~
- ~~pending activation の public `IdentityLookup::resolve` contract を固定する。~~
- ~~topology update 後に absent authority の activation / cache が無効化されることを固定する。~~
- ~~leave / down / passivation と `GrainRef` 解決の関係を固定する。~~

### ~~3. Provider boundary hardening~~

- ~~local / static / AWS ECS provider がどこまで membership を供給し、どこから cluster core が扱うかを文書化する。~~
- ~~seed / discovery / lifecycle adapter の責務境界を明確にする。~~
- ~~std adapter 実装で保持すべき subscription / driver lifetime を確認する。~~

作業メモ: [2026-05-26_cluster-provider-boundary.md](2026-05-26_cluster-provider-boundary.md) で、DIP と port-and-adapter の向きを `cluster-core` が policy / port を所有し std が adapter 実装に留まる形として整理する。

### ~~4. Failure detector and downing minimum~~

- ~~`DowningProvider` を単なる explicit down hook から、failure observation に対する判断 contract へ拡張するか検討する。~~
- ~~SBR 全面実装ではなく、最小 downing decision model を先に切る。~~
- ~~Reachability matrix を入れるか、現在の suspect / unreachable event model を強化するかを比較する。~~

作業メモ: [2026-05-26_failure-downing-boundary.md](2026-05-26_failure-downing-boundary.md) で、failure observation と member departure input を分離し、`DowningProvider` を decision port として扱う最小 contract を整理する。

### ~~5. Placement scalability~~

- ~~Rendezvous hashing のまま伸ばす範囲を明確にする。~~
- ~~rebalance を即実装する前に、join / leave / rolling update 時の movement と cache invalidation の期待値を固定する。~~
- ~~remembered entities は persistence integration の要求が明確になるまで deferred とする。~~

作業メモ: `define-placement-movement-contract` と `test-grain-pending-activation-contract` により、現行の bounded Placement scalability contract は `cluster-grain-runtime-operational-contract` に集約済み。今後の rebalance / remembered entities / recovery / drain は別 change として扱う。

## Deferred scope

以下は直近 roadmap から外す。必要になった時点で OpenSpec change として個別に切る。

- typed Cluster API wrapper
- Cluster Singleton / ShardCoordinator parity
- Cluster Client / Receptionist
- DistributedData / CRDT Replicator
- sharding delivery controllers
- replicated sharding / direct replication
- Pekko serializer binary compatibility
- least-shard rebalance / minimum movement guarantee
- remembered entities / persistence-backed activation recovery
- in-flight request draining

## OpenSpec 境界

この文書は方針とロードマップであり、API 仕様変更ではない。`ClusterProvider`、`DowningProvider`、`IdentityLookup`、`PlacementCoordinatorCore` などの公開 contract を変える段階で、個別の OpenSpec change を作成する。
