# 要件ドキュメント

## 導入

この仕様は、`docs/gap-analysis/cluster-gap-analysis.md` の active follow-up のうち、Distributed PubSub mediator protocol に属する項目を cluster pub-sub contract として固定する。対象は `DistributedPubSubMediator` protocol、`DistributedPubSubSettings`、`Send` / `SendToAll` path semantics、topic registry gossip / delta collection に限定する。

この仕様は `cluster-membership-reachability-model` の membership identity / active member view と、`cluster-gossip-heartbeat-protocol` の gossip envelope / dissemination substrate を利用する。gossip substrate 本体、downing SBR、discovery provider、cluster message serialization contract、Distributed Data / CRDT はこの仕様では実装しない。

## 要件

### 要件1: DistributedPubSubMediator protocol

**目的:** cluster pub-sub 実装者として、local mediator に対する register / subscribe / publish / query command を明確な protocol として扱い、topic delivery と path delivery の入力を混同せずに検証したい。

#### 受け入れ条件

1. actor が mediator へ `Put` 相当の登録を要求したとき、PubSub Mediator Protocol は actor path と local actor target を topic registry とは別の path registry entry として保持しなければならない。
2. actor が mediator へ `Remove` 相当の削除を要求したとき、PubSub Mediator Protocol は対象 path の削除を registry delta で観測可能にしなければならない。
3. subscriber が topic へ `Subscribe` を要求したとき、PubSub Mediator Protocol は topic、optional group、subscriber target を保持し、成功時に subscribe acknowledgement を返さなければならない。
4. subscriber が topic から `Unsubscribe` を要求したとき、PubSub Mediator Protocol は対象 subscription の削除を registry delta で観測可能にし、成功時に unsubscribe acknowledgement を返さなければならない。
5. publisher が topic publish を要求したとき、PubSub Mediator Protocol は topic subscriber registry に基づく delivery intent を生成し、path registry の `Send` semantics と混同してはならない。
6. current topics または subscriber count が要求されたとき、PubSub Mediator Protocol は registry snapshot から観測可能な query result を返さなければならない。

### 要件2: DistributedPubSubSettings

**目的:** cluster operator と runtime 実装者として、mediator の role filter、routing、gossip interval、delta chunking、removed entry retention を設定値として観測し、protocol behavior を ad hoc 定数に依存させたくない。

#### 受け入れ条件

1. mediator settings が生成されるとき、PubSub Settings は role filter、routing mode、gossip interval、removed entry TTL、max delta elements、no-subscriber behavior を保持しなければならない。
2. role filter を含む場合、PubSub Settings は membership view から該当 role の active member だけを mediator gossip target 候補にしなければならない。
3. routing mode が `Send` delivery に使われる場合、PubSub Settings は one-of selection 用の random または round-robin として観測可能にしなければならない。
4. unsupported routing mode が指定された場合、PubSub Settings は caller から観測可能な configuration error として扱わなければならない。
5. max delta elements が設定されている間、PubSub Settings は 1回の registry delta collection がその上限を超えないようにし続けなければならない。
6. no-subscriber behavior が dead letters を要求する場合、PubSub Settings は delivery target が存在しない publish / send を観測可能な dead-letter intent に変換しなければならない。

### 要件3: Send / SendToAll path semantics

**目的:** actor path delivery の利用者として、topic publish とは別に、cluster 全体の同じ actor path へ one-of または all-of delivery を指定し、local affinity と self-skip を期待通りに扱いたい。

#### 受け入れ条件

1. `Send` が actor path と message を受け取ったとき、PubSub Mediator Protocol は matching path registry entry のうち1つへ routing mode に従う delivery intent を生成しなければならない。
2. `Send` が local affinity を指定している場合、PubSub Mediator Protocol は local owner の matching entry を優先し、存在しない場合だけ cluster-wide entry を候補にしなければならない。
3. `SendToAll` が actor path と message を受け取ったとき、PubSub Mediator Protocol は matching path registry entry すべてへ delivery intent を生成しなければならない。
4. `SendToAll` が all-but-self を指定している場合、PubSub Mediator Protocol は local owner の matching entry を delivery target から除外しなければならない。
5. path が空または message が mediator payload として扱えない場合、PubSub Mediator Protocol は caller から観測可能な validation failure として扱わなければならない。
6. matching path registry entry が存在しない場合、PubSub Mediator Protocol は settings の no-subscriber behavior に従って drop または dead-letter intent を生成しなければならない。

### 要件4: topic registry gossip / delta collection

**目的:** cluster pub-sub 実装者として、各 node の topic / path registry を bounded delta として交換し、古い削除済み entry の再出現や過大 payload を防ぎたい。

#### 受け入れ条件

1. local registry が変更されたとき、Topic Registry は owner identity、monotonic version、entry key、entry value を持つ bucket update として保持しなければならない。
2. remote registry status を受け取ったとき、Topic Registry は相手の owner version map と local bucket version を比較して、不足分だけを delta として収集しなければならない。
3. delta collection が max delta elements に到達した場合、Topic Registry は version order が低い entry から bounded chunk を生成しなければならない。
4. remove entry が生成された場合、Topic Registry は removed entry TTL が満了するまで tombstone として観測可能にしなければならない。
5. removed entry TTL が満了した場合、Topic Registry は convergence と retention rule に従って tombstone を prune できなければならない。
6. unknown owner または non-active member から registry delta を受け取った場合、Topic Registry は caller から観測可能な ignored outcome として扱い、削除済み member の entry を復活させてはならない。

### 要件5: membership / gossip integration

**目的:** downstream spec 実装者として、pubsub registry dissemination が membership と gossip substrate を利用しつつ、それらの merge semantics や transport lifecycle を所有しないことを確認したい。

#### 受け入れ条件

1. membership current state が更新されたとき、PubSub Mediator Protocol は role filter と active member status に従って mediator peer set を更新しなければならない。
2. member が removed / downed / left と観測された場合、PubSub Mediator Protocol はその owner の registry bucket を delivery candidate から外さなければならない。
3. registry gossip tick が発生したとき、PubSub Mediator Protocol は gossip substrate が運べる pubsub registry status または delta payload を生成しなければならない。
4. pubsub registry payload が gossip envelope に載る場合、PubSub Mediator Protocol は payload kind と registry version を提供し、gossip envelope framing や heartbeat scheduling を所有してはならない。
5. reachability evidence が変化した場合、PubSub Mediator Protocol は delivery candidate の評価に必要な view だけを参照し、reachability matrix の merge または downing decision を実行してはならない。

### 要件6: std adaptor boundary and scope evidence

**目的:** cluster roadmap の実装者として、この仕様が pubsub mediator protocol だけを完了し、std delivery、serialization、gossip substrate、downing、discovery の責務を吸収しないことを確認したい。

#### 受け入れ条件

1. local delivery が std runtime で実行されるとき、Std PubSub Adaptor は core が生成した delivery intent を実行し、mediator protocol semantics を変更してはならない。
2. pubsub message serialization が必要な場合、PubSub Mediator Protocol は existing actor serialization extension の利用に留め、cluster message serializer framework を定義してはならない。
3. core pubsub contract が追加される間、PubSub Mediator Protocol は `no_std` + `alloc` 境界を維持し、Tokio、network I/O、host clock を core に持ち込んではならない。
4. downing SBR または discovery provider が必要な場合、PubSub Mediator Protocol は該当 downstream spec の責務として残さなければならない。
5. gap analysis を更新するとき、PubSub Mediator Protocol は `DistributedPubSubMediator` protocol、`DistributedPubSubSettings`、`Send` / `SendToAll` path semantics、topic registry gossip / delta collection だけを完了候補として扱わなければならない。
