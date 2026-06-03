# 要件ドキュメント

## 導入

この仕様は、`docs/gap-analysis/cluster-gap-analysis.md` の active medium follow-up のうち、membership / reachability model に属する項目を core contract として固定する。対象は `UniqueAddress` semantics、data center membership、`WeaklyUp` compatibility、`Reachability` matrix、indirect connection handling に限定する。

この仕様は、後続の gossip / heartbeat、downing SBR、discovery provider、pubsub、serialization の前提になる membership snapshot と reachability evidence を提供する。これら後続機能の protocol、transport、wire format、decision policy はこの仕様では実装しない。

## 要件

### 要件1: UniqueAddress semantics

**目的:** cluster runtime 実装者として、同じ authority を再利用する node incarnation を誤って同一 member と扱わず、membership と reachability の観測結果を正しい node identity に紐づけたい。

#### 受け入れ条件

1. node が membership に参加するとき、Membership Model は address と uid の組を member identity として保持しなければならない。
2. 同じ address で異なる uid の node が参加する場合、Membership Model は別 incarnation として観測可能にしなければならない。
3. 同じ address と uid の node が再観測された場合、Membership Model は既存 member identity への再観測として扱わなければならない。
4. membership snapshot または delta が生成されるとき、Membership Model は member identity を失わずに含めなければならない。
5. uid が未確定の identity を受け取った場合、Membership Model は caller から観測可能な失敗または未確定状態として扱い、確定済み member と同一視してはならない。

### 要件2: data center membership

**目的:** cluster runtime 実装者として、member が属する data center を membership view で扱い、後続の Cross-DC heartbeat や routing policy が同じ data center 語彙を参照できるようにしたい。

#### 受け入れ条件

1. node が membership に参加するとき、Membership Model は data center を member record に保持しなければならない。
2. data center が指定されていない場合、Membership Model は明示的な default data center として観測可能にしなければならない。
3. membership snapshot または current cluster state が生成されるとき、Membership Model は各 member の data center を保持しなければならない。
4. data center ごとの member view が要求されたとき、Membership Model は member status と reachability evidence を失わずに絞り込めなければならない。
5. data center membership が存在する間、Membership Model は Cross-DC heartbeat protocol を実装したものとして振る舞ってはならない。

### 要件3: WeaklyUp compatibility

**目的:** cluster runtime 実装者として、Pekko comparison で必要な `WeaklyUp` 相当の member status を扱い、join 直後の暫定参加状態と通常の `Up` を区別したい。

#### 受け入れ条件

1. join 中の member が暫定参加として受理されたとき、Membership Model は `WeaklyUp` 相当の status を観測可能にしなければならない。
2. `WeaklyUp` member が通常参加へ昇格するとき、Membership Model は `WeaklyUp` から `Up` への status transition を生成しなければならない。
3. `WeaklyUp` member が leave または down される場合、Membership Model は transition を status rule に従って生成しなければならない。
4. active member view が要求されたとき、Membership Model は `WeaklyUp` を暫定参加として含めるかどうかを caller が判定できる状態で公開しなければならない。
5. `WeaklyUp` compatibility が存在する間、Membership Model は split-brain resolution の decision を行ってはならない。

### 要件4: Reachability matrix

**目的:** downing や pubsub の後続実装者として、単一 node の `Suspect` 状態ではなく、observer / subject / status / version を持つ reachability evidence を参照したい。

#### 受け入れ条件

1. observer が subject を unreachable と観測したとき、Reachability Model は observer、subject、status、version を持つ record として保持しなければならない。
2. observer が subject を reachable と観測したとき、Reachability Model は該当 observer row の version を進め、全 subject が reachable なら不要な reachable record を保持し続けてはならない。
3. observer が subject を terminated と観測したとき、Reachability Model は terminated を unreachable より強い集約状態として扱わなければならない。
4. 複数 observer の record が存在する場合、Reachability Model は subject ごとの aggregated status を観測可能にしなければならない。
5. reachability snapshot または membership snapshot が生成されるとき、Reachability Model は matrix version と records を失わずに含めなければならない。

### 要件5: indirect connection handling

**目的:** downing decision の後続実装者として、直接 failure detector だけでは判断できない indirect connection 状態を、membership core から evidence として受け取りたい。

#### 受け入れ条件

1. observer と subject の reachability record が更新されたとき、Membership Model は direct observation と indirect observation を区別できる evidence を生成しなければならない。
2. ある subject が一部 observer から unreachable で、別 observer から reachable と観測される場合、Membership Model は partial connectivity として観測可能にしなければならない。
3. observer 自身が unreachable または terminated と集約される場合、Membership Model はその observer の record を indirect connection 判定で区別できる状態にしなければならない。
4. indirect connection evidence が downing boundary へ渡されるとき、Membership Model は downing decision を実行せず、入力 evidence だけを渡さなければならない。
5. indirect connection evidence が存在しない場合、Membership Model は direct reachability evidence だけで観測結果を表現しなければならない。

### 要件6: scope protection and evidence

**目的:** cluster roadmap の実装者として、この仕様が membership / reachability の基礎 model だけを完了し、後続 spec の責務を吸収しないことを確認したい。

#### 受け入れ条件

1. gossip envelope、heartbeat protocol、full gossip merge、tombstone、seen digest が必要な場合、Membership Reachability Model は `cluster-gossip-heartbeat-protocol` の責務として残さなければならない。
2. SplitBrainResolver、DowningStrategy、lease-based majority が必要な場合、Membership Reachability Model は `cluster-downing-sbr-decision-model` の責務として残さなければならない。
3. discovery provider、SeedNodeProcess、pubsub mediator、message serialization が必要な場合、Membership Reachability Model は該当 downstream spec の責務として残さなければならない。
4. gap analysis を更新するとき、Membership Reachability Model は `UniqueAddress` semantics、data center membership、`WeaklyUp` compatibility、`Reachability` matrix、indirect connection handling だけを完了候補として扱わなければならない。
