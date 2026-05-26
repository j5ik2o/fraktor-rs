# cluster provider boundary

## 目的

`cluster-*` の provider は、`cluster-core` が定義する port を通じて各 discovery / lifecycle source を topology input に正規化する境界である。DIP と port-and-adapter の向きは、core が policy と port を所有し、std が adapter として実装する形を維持する。Grain runtime は provider 種別を知らず、`TopologyUpdated` や member departure input に基づいて identity lookup、placement、activation / PID cache invalidation を行う。

この文書は `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` の Task slice 3 に対応する。Downing decision model、reachability matrix、rebalance、remembered entities は次の change に分ける。

進捗: `document-cluster-provider-boundary` として完了済み。

## 境界

### cluster-core

`cluster-core` が所有するもの:

- ~~`ClusterProvider` trait の共通操作。~~
- ~~provider port と extension lifecycle policy。~~
- ~~`LocalClusterProvider` / `StaticClusterProvider` の no_std compatible な topology publication。~~
- ~~`ClusterEvent::TopologyUpdated`、`TopologyUpdate`、`ClusterTopology` の表現。~~
- ~~Grain runtime 側の topology invalidation contract。~~

`cluster-core` が所有しないもの:

- remoting lifecycle event subscription。
- AWS ECS API polling。
- provider-specific discovery backend。
- failure observation から downing decision を作る policy。
- rebalance、remembered entities、in-flight drain。

### LocalClusterProvider

Local provider は core-defined `ClusterProvider` port の実装である。明示的な `join` / `leave` / `down` と、std adapter が外部 remoting lifecycle から変換した provider port input を membership input として扱う。

- ~~`start_member` は自身の advertised address を members に追加し、startup event を publish する。~~
- ~~`start_client` は client startup event を publish する。~~
- ~~static topology が設定されている場合、start 時に topology update を publish する。~~
- ~~`join` は advertised address 自身を no-op とし、それ以外の authority を joined topology input に変換する。~~
- ~~`leave` / `down` は current member を left topology input に変換する。~~

### StaticClusterProvider

Static provider は discovery を持たない。設定済み topology を start 時に EventStream へ publish するだけの provider である。

- ~~`start_member` / `start_client` は設定済み topology を topology update として publish する。~~
- ~~configured topology がなければ topology update は publish しない。~~
- ~~`join` / `leave` / `down` は topology discovery を行わない。~~
- ~~shutdown に追加の cleanup はない。~~

### std remoting lifecycle adapter

`cluster-adaptor-std` の remoting lifecycle adapter は std-only の adapter である。core policy を所有せず、core-defined provider port へ外部 lifecycle signal を供給するための subscription lifetime だけを持つ。

- ~~`subscribe_remoting_events` は `RemotingLifecycleEvent::Connected` を local provider の join input に変換する。~~
- ~~`RemotingLifecycleEvent::Quarantined` は local provider の leave input に変換する。~~
- ~~provider 起動前の lifecycle event は無視する。~~
- ~~返された `EventStreamSubscription` を保持している間だけ adapter は有効である。~~
- ~~subscription は provider を強参照し続けない。~~

### AwsEcsClusterProvider

AWS ECS provider は `cluster-adaptor-std` 側の `ClusterProvider` adapter 実装であり、AWS ECS task discovery を core-defined provider port の topology input に変換する。

- ~~`start_member` は自身の advertised address を members に追加し、初回 topology update と startup event を publish し、poller を開始する。~~
- ~~`start_client` は自身を member に加えず、startup event を publish し、poller を開始する。~~
- ~~poller は ECS `ListTasks` / `DescribeTasks` の結果から running task の private IPv4 address を authority candidate に変換する。~~
- ~~discovered member 差分を joined / left topology update として publish する。~~
- ~~`down` は known remote member を left topology input に変換するが、自分自身の down は拒否する。~~
- ~~`join` / `leave` は明示操作としては非対応で、discovery source は ECS polling に限定する。~~
- ~~`shutdown` は poller stop signal を立て、members を clear し、shutdown event を publish する。~~

## 後続

次に切るべき change は `define-minimum-downing-decision-contract` である。そこでは provider が departure input を出す前段として、failure observation、suspect / unreachable 表現、explicit down hook、downing decision の責務を分けて扱う。
