## Context

`cluster-*` は Virtual Actor / Grain runtime を主軸にする。現在の実装は `PartitionIdentityLookup`、`PlacementCoordinatorCore`、`VirtualActorRegistry`、`PidCache` により、Grain key から authority と PID を決定し、topology update、member departure、passivation に応じて activation / cache を無効化できる。

一方で、これらは個別 unit test として存在しており、運用時に守る contract としてはまだまとまっていない。次の実装では、新しい大規模機能を追加する前に、identity resolution、placement cache、topology update、rolling update 時の期待値を contract test と仕様で固定する。

## Goals / Non-Goals

**Goals:**

- Grain identity resolution の normal / pending / no authority / cache hit を仕様化し、contract test で固定する。
- topology update と member departure が absent authority の activation / PID cache を無効化することを仕様化する。
- passivation 後の再解決と cache invalidation event の観測点を固定する。
- rolling update を join -> topology update -> leave/down -> invalidation -> re-resolution の contract として説明できる状態にする。
- `no_std` core の `identity` / `placement` / `grain` を中心に閉じ、std adaptor は必要な観測境界だけを扱う。

**Non-Goals:**

- Split Brain Resolver の本実装。
- reachability matrix の導入。
- shard rebalance strategy の導入。
- remembered entities と persistence integration。
- Pekko typed Cluster API、Cluster Singleton、ShardCoordinator、DistributedData parity。
- provider-specific discovery backend の拡張。

## Decisions

### Decision 1: Operational contract を core-first で固定する

`PartitionIdentityLookup` と `PlacementCoordinatorCore` の contract test を優先する。これにより、std adaptor や provider ごとの差異に引きずられず、Grain runtime の中核 contract を `no_std` core で検証できる。

Alternative: `cluster-adaptor-std` の end-to-end test から始める。これは実運用に近いが、transport、executor、provider lifecycle の失敗と placement contract の失敗が混ざるため、最初の change としては範囲が広すぎる。

### Decision 2: topology change は cache invalidation contract として扱う

authority set が変わった場合、absent authority の activation / PID cache は再利用してはならない。Rendezvous hashing の movement 最適化や rebalance はこの change では扱わず、「消えた authority を指す解決結果を返さない」ことを先に固定する。

Alternative: rebalance semantics まで同時に設計する。これは大規模運用には重要だが、movement 量、drain、remembered entities の判断が必要になり、直近の contract 固定から逸れる。

### Decision 3: rolling update は minimum guarantee だけ定義する

rolling update 時の保証は、departed authority の stale PID を再利用しないこと、新しい topology に基づいて再解決すること、passivated activation を cache hit として返さないことに限定する。in-flight request の完全 drain、remembered activation 復元、zero-movement rebalance は明示的に非対象とする。

Alternative: rolling update 全体の orchestration を provider lifecycle として定義する。これは後続 change で扱うべきであり、今回の scope では provider-specific な差異が大きい。

### Decision 4: failure detector / downing は入力境界として残す

この change では、failure detector や `DowningProvider` が最終的に topology update / member departure を発生させる前提だけを置く。downing decision model 自体は次の change に分ける。

Alternative: SBR の最小 decision model を同時に導入する。これは `Reachability` 表現の選択に依存するため、Grain runtime contract 固定後に切る方がよい。

## Risks / Trade-offs

- [Risk] core contract だけでは provider lifecycle の抜けが残る -> Mitigation: std adaptor の task は smoke / boundary test に限定し、provider lifecycle hardening は別 change に分ける。
- [Risk] 既存 test と重複する -> Mitigation: 既存 unit test は部品検証として残し、新規 contract test は join / leave / down / rolling update のシナリオ名で意図を固定する。
- [Risk] topology update の semantics が rebalance 期待と混同される -> Mitigation: spec で rebalance / remembered entities を非対象として明記する。
- [Risk] pending resolution の非同期 activation flow が曖昧になる -> Mitigation: distributed activation enabled 時の `Pending` と command result 後の resolution を同じ requirement に含める。
