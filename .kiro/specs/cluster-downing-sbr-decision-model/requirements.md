# 要件ドキュメント

## 導入

この feature は、membership reachability model が提供する member snapshot と reachability evidence を入力に、Split Brain Resolver と DowningStrategy の decision contract を定義する。目的は、core が downing decision semantics を所有し、std/provider 側が lifecycle と lease backend binding だけを担当できるようにすることである。

対象範囲は `SplitBrainResolver`、`DowningStrategy` decision model、lease-based majority、provider-facing SBR integration に限定する。gossip heartbeat、discovery provider、pubsub mediator、cluster message serialization は隣接 spec に残す。

## 要件

### 要件1: Downing decision input
**目的:** cluster runtime 実装者として、membership snapshot と reachability evidence を同じ downing input として扱い、downing decision が観測可能な根拠に基づく状態を得たい。

#### 受け入れ条件
1. membership snapshot と reachability evidence が提示されるイベントが起きたとき、Downing/SBR decision model は decision 対象 member、observer evidence、member status、data center を同じ評価入力として保持しなければならない
2. reachability evidence が不足している状態の場合、Downing/SBR decision model は不十分な根拠として decision を保留しなければならない
3. explicit down command が提示されるイベントが起きたとき、Downing/SBR decision model は membership reachability 評価を要求せず explicit decision として扱わなければならない
4. upstream membership が `WeaklyUp`、unreachable、terminated を含む場合、Downing/SBR decision model はそれらを区別できる入力語彙を保持しなければならない
5. Downing/SBR decision model は常に gossip merge、heartbeat scheduling、discovery lookup を実行しない入力 contract に留まらなければならない

### 要件2: Split Brain Resolver strategy decision
**目的:** cluster runtime 実装者として、Split Brain Resolver の strategy ごとに decision と理由を検証し、partition handling の意味を再現可能にしたい。

#### 受け入れ条件
1. strategy evaluation が起きたとき、Split Brain Resolver は `KeepMajority`、`LeaseMajority`、`StaticQuorum`、`KeepOldest`、`DownAll` の strategy identity を区別して decision を返さなければならない
2. stable-after の待機条件が満たされていない状態の場合、Split Brain Resolver は unstable decision として保留しなければならない
3. majority partition が一意に決まる状態の場合、Split Brain Resolver は保持する partition と downing 対象 member を decision trace に含めなければならない
4. majority partition が同数または曖昧な状態の場合、Split Brain Resolver は deterministic tie-break rule または defer reason を decision trace に含めなければならない
5. `DownAll` strategy を含む場合、Split Brain Resolver は down-all timeout が満たされた場合だけ all-down decision を返さなければならない

### 要件3: Lease-based majority
**目的:** operator と runtime 実装者として、lease backend の取得結果が majority decision に反映され、lease なしの誤 downing を避けたい。

#### 受け入れ条件
1. `LeaseMajority` strategy が評価されるイベントが起きたとき、Downing/SBR decision model は majority partition が lease acquisition に成功した場合だけ keep decision を返さなければならない
2. lease acquisition が失敗した状態の場合、Downing/SBR decision model は lease failure reason を含む defer または down decision を返さなければならない
3. lease backend が未設定の場合、Downing/SBR decision model は `LeaseMajority` を設定不備として観測可能な failure にしなければならない
4. lease backend が一時的に判断不能な状態の場合、Downing/SBR decision model は member state を変更せず decision を保留しなければならない
5. Downing/SBR decision model は常に lease backend の具体実装、network I/O、host clock ownership を core に持ち込んではならない

### 要件4: Provider-facing SBR integration
**目的:** provider 実装者として、Split Brain Resolver を downing provider lifecycle に接続し、provider 側が decision semantics を重複実装しない状態にしたい。

#### 受け入れ条件
1. cluster provider が SBR provider を構成するイベントが起きたとき、provider-facing integration は provider key、SBR settings、strategy identity を compatibility metadata として公開しなければならない
2. provider lifecycle が開始されるイベントが起きたとき、provider-facing integration は core SBR evaluator を provider decision hook へ接続しなければならない
3. provider lifecycle が停止または破棄された状態の場合、provider-facing integration は pending lease operation や decision request を provider lifetime の外へ残してはならない
4. provider-facing integration で decision failure が起きたとき、provider は failure reason を観測可能な error として返さなければならない
5. provider-facing integration は常に std/provider の lifecycle と lease backend binding を担当し、downing decision semantics を所有してはならない

### 要件5: Boundary and downstream compatibility
**目的:** spec reviewer と downstream spec 実装者として、この spec が downing/SBR 以外の follow-up を吸収しないことを確認し、後続 workstream の責務を保ちたい。

#### 受け入れ条件
1. gossip heartbeat、GossipEnvelope、CrossDcClusterHeartbeat に関する変更が必要になった状態の場合、この spec はそれらを out of boundary として扱わなければならない
2. discovery provider、SeedNodeProcess、generic discovery adapter に関する変更が必要になった状態の場合、この spec はそれらを out of boundary として扱わなければならない
3. DistributedPubSubMediator、pubsub topic registry、cluster message serializer に関する変更が必要になった状態の場合、この spec はそれらを out of boundary として扱わなければならない
4. upstream membership reachability contract が変わった状態の場合、Downing/SBR decision model は revalidation 対象として扱われなければならない
5. docs gap analysis を更新する操作を含む場合、更新対象は downing/SBR decision model、SplitBrainResolver、DowningStrategy、lease-based majority、provider-facing SBR integration に限定しなければならない
