# 要件ドキュメント

## 導入

この仕様は、`docs/gap-analysis/cluster-gap-analysis.md` の active follow-up のうち、gossip と heartbeat protocol に属する項目を cluster membership の contract として固定する。対象は `GossipEnvelope`、dedicated cluster heartbeat protocol、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat` に限定する。

この仕様は `cluster-membership-reachability-model` が提供する `UniqueAddress`、data center、reachability matrix を前提にする。downing SBR、discovery provider、pubsub mediator、cluster message serialization contract は後続または隣接 spec の責務として残す。

## 要件

### 要件1: GossipEnvelope

**目的:** cluster runtime 実装者として、gossip payload の送信元、送信先、deadline、payload identity を観測できる envelope を持ち、transport と membership semantics を混同せずに扱いたい。

#### 受け入れ条件

1. gossip payload が peer へ送信されるとき、Gossip Protocol は送信元 `UniqueAddress`、送信先 `UniqueAddress`、payload kind、membership version を持つ envelope として表現しなければならない。
2. gossip envelope が生成されるとき、Gossip Protocol は payload の意味が membership delta、full gossip state、seen digest、heartbeat evidence のいずれかとして判定できる状態にしなければならない。
3. envelope deadline を過ぎた outbound payload が dispatch 対象に含まれる場合、Gossip Protocol は caller から観測可能な expired outcome として扱い、期限切れ payload を正常送信扱いにしてはならない。
4. envelope の送信元または送信先 identity が未確定の場合、Gossip Protocol は caller から観測可能な失敗として扱い、authority 文字列だけで identity を補完してはならない。
5. std transport が logical envelope を受け渡す間、Gossip Protocol は envelope の identity と payload kind を失わずに core へ戻さなければならない。

### 要件2: full Gossip merge / tombstone / seen digest

**目的:** cluster runtime 実装者として、delta diffusion だけではなく full gossip state の merge、removed member の tombstone、convergence 判定用 seen digest を扱い、membership convergence を説明可能にしたい。

#### 受け入れ条件

1. full gossip state を受け取ったとき、Gossip Protocol は local membership state と remote gossip state を version と identity に基づいて merge しなければならない。
2. 同じ member identity に競合する record が存在する場合、Gossip Protocol は deterministic な precedence rule で merge result を生成しなければならない。
3. member が removed または dead になった場合、Gossip Protocol は再出現を防ぐ tombstone を観測可能にしなければならない。
4. tombstone retention 条件が満たされた場合、Gossip Protocol は convergence と retention rule に従って prune できなければならない。
5. gossip state が peer に観測されたとき、Gossip Protocol は seen digest を更新し、どの peer がどの version を確認したかを観測可能にしなければならない。
6. 全 active peer が対象 version を確認した場合、Gossip Protocol は convergence を観測可能にしなければならない。

### 要件3: dedicated cluster heartbeat protocol

**目的:** cluster runtime 実装者として、gossip delta の副作用ではなく dedicated heartbeat request / response によって liveness evidence を交換し、failure detector と reachability matrix の入力を分離したい。

#### 受け入れ条件

1. heartbeat tick が発生したとき、Cluster Heartbeat Protocol は peer ごとに sequence number を持つ heartbeat request を生成しなければならない。
2. heartbeat request を受け取ったとき、Cluster Heartbeat Protocol は送信元 identity と sequence number を保持した heartbeat response を生成しなければならない。
3. heartbeat response を受け取ったとき、Cluster Heartbeat Protocol は対応する request と照合し、liveness evidence を観測可能にしなければならない。
4. first heartbeat expectation の期限内に response がない場合、Cluster Heartbeat Protocol は caller から観測可能な missed heartbeat evidence を生成しなければならない。
5. heartbeat evidence が membership core へ渡されるとき、Cluster Heartbeat Protocol は reachability update の入力だけを渡し、downing decision を実行してはならない。

### 要件4: CrossDcClusterHeartbeat

**目的:** multi data center cluster の実装者として、local data center heartbeat と cross data center heartbeat を区別し、data center をまたぐ liveness evidence を独立して観測したい。

#### 受け入れ条件

1. peer が local data center と異なる場合、Cross-DC Heartbeat Protocol は cross-DC heartbeat 対象として観測可能にしなければならない。
2. cross-DC heartbeat tick が発生したとき、Cross-DC Heartbeat Protocol は local heartbeat と区別できる heartbeat request を生成しなければならない。
3. cross-DC heartbeat response を受け取ったとき、Cross-DC Heartbeat Protocol は data center pair と peer identity を保持した liveness evidence を生成しなければならない。
4. data center membership が更新された場合、Cross-DC Heartbeat Protocol は heartbeat 対象の追加、削除、維持を観測可能にしなければならない。
5. Cross-DC heartbeat evidence が存在する間、Cross-DC Heartbeat Protocol は routing policy、discovery provider、downing strategy を決定してはならない。

### 要件5: std transport handoff integration

**目的:** std adaptor 実装者として、core の gossip semantics を std 側へ移さず、logical envelope handoff と transport lifecycle だけを std adaptor の責務として扱いたい。

#### 受け入れ条件

1. gossip envelope が std transport へ渡されるとき、Std Gossip Transport は identity と payload kind を保持した logical transport handoff として扱わなければならない。
2. logical transport payload を受け取ったとき、Std Gossip Transport は不正な payload kind、identity、unknown peer を caller から観測可能な transport failure として扱わなければならない。
3. std transport が heartbeat payload を送受信する場合、Std Gossip Transport は heartbeat request / response と gossip state payload を区別しなければならない。
4. std transport が peer list を更新する場合、Std Gossip Transport は envelope の target identity と transport endpoint の対応を失わずに更新しなければならない。
5. std transport が payload を処理する間、Std Gossip Transport は merge rule、tombstone rule、seen digest rule、reachability decision を所有してはならない。

### 要件6: scope protection and evidence

**目的:** cluster roadmap の実装者として、この仕様が gossip / heartbeat protocol だけを完了し、隣接 spec の責務を吸収しないことを確認したい。

#### 受け入れ条件

1. SplitBrainResolver、DowningStrategy、lease-based majority が必要な場合、Gossip Heartbeat Protocol は `cluster-downing-sbr-decision-model` の責務として残さなければならない。
2. SeedNodeProcess または generic discovery adapter が必要な場合、Gossip Heartbeat Protocol は `cluster-discovery-provider-interop` の責務として残さなければならない。
3. DistributedPubSubMediator、topic registry gossip、delta collection が必要な場合、Gossip Heartbeat Protocol は `cluster-pubsub-mediator-protocol` の責務として残さなければならない。
4. cluster message serializer framework が必要な場合、Gossip Heartbeat Protocol は `cluster-message-serialization-contract` の責務として残さなければならない。
5. gap analysis を更新するとき、Gossip Heartbeat Protocol は `GossipEnvelope`、dedicated cluster heartbeat protocol、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat` だけを完了候補として扱わなければならない。
