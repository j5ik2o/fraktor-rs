# 要件ドキュメント

## 導入

この仕様は、`docs/gap-analysis/cluster-gap-analysis.md` の active medium follow-up に残っている `SeedNodeProcess` と generic discovery adapter を、cluster provider boundary の一部として定義する。目的は、seed node discovery と discovery backend の結果を provider-neutral な topology input へ変換し、cluster core の placement / membership から provider-specific discovery details を切り離すことである。

この仕様は discovery provider interop に限定する。membership reachability、gossip heartbeat、downing / SBR decision、pubsub mediator、cluster message serialization、特定 cloud provider の完全互換は扱わない。

## 要件

### 要件1: SeedNodeProcess の起動入力

**目的:** std cluster provider 利用者として、静的 seed node と discovery 由来 seed node を同じ起動入力として扱い、provider 起動時の join input を一貫して観測したい。

#### 受け入れ条件

1. seed node source が node authority を返したとき、Cluster Discovery Provider Interop は provider-neutral な join input として topology update へ変換しなければならない。
2. seed node source が local advertised authority を含む場合、Cluster Discovery Provider Interop は self join を重複した remote join input として扱ってはならない。
3. seed node source が空の場合、Cluster Discovery Provider Interop は cluster startup を失敗させず、明示 seed なしの provider lifecycle として観測可能にしなければならない。
4. seed node source が invalid authority を返した場合、Cluster Discovery Provider Interop は caller または operator から観測可能な discovery failure として報告しなければならない。
5. provider が shutdown された場合、Cluster Discovery Provider Interop は SeedNodeProcess 由来の追加 join input を生成し続けてはならない。

### 要件2: generic discovery adapter の正規化

**目的:** discovery backend 実装者として、backend 固有の結果を cluster core に漏らさず、authority 候補と lifecycle outcome だけを provider boundary に渡したい。

#### 受け入れ条件

1. discovery backend が endpoint 群を返したとき、Cluster Discovery Provider Interop は authority、source identity、observation time を含む provider-neutral discovery result として表現しなければならない。
2. discovery backend が一時的に失敗した場合、Cluster Discovery Provider Interop は既存 topology を破壊せず、failure を provider lifecycle から観測可能にしなければならない。
3. discovery backend が同じ authority を複数返した場合、Cluster Discovery Provider Interop は重複しない authority set として topology input を生成しなければならない。
4. discovery backend が provider-specific metadata を返した場合、Cluster Discovery Provider Interop はその metadata を placement / membership policy の入力にしてはならない。
5. discovery backend を差し替える場合、Cluster Discovery Provider Interop は Local / static / AWS ECS provider の既存 public behavior を壊してはならない。

### 要件3: provider lifecycle と topology publication

**目的:** std adaptor 利用者として、discovery polling または subscription の lifetime が provider lifecycle に従い、start / refresh / shutdown の境界で topology update が過不足なく生成されることを期待したい。

#### 受け入れ条件

1. provider が member mode で start したとき、Cluster Discovery Provider Interop は configured seed と discovery result を join topology input に変換しなければならない。
2. provider が client mode で start したとき、Cluster Discovery Provider Interop は full member として自己登録する topology input を生成してはならない。
3. discovery result が refresh されたとき、Cluster Discovery Provider Interop は joined / left の差分だけを topology update として観測可能にしなければならない。
4. provider shutdown が完了した場合、Cluster Discovery Provider Interop は discovery subscription または polling task を停止した状態として観測可能にしなければならない。
5. discovery lifecycle が provider より長く残る場合、Cluster Discovery Provider Interop は provider を strong reference で生存させ続けてはならない。

### 要件4: topology input 変換境界

**目的:** cluster core / Grain runtime 実装者として、provider-specific discovery details を読まずに、正規化済み topology input だけで placement invalidation と membership bootstrap を扱いたい。

#### 受け入れ条件

1. discovery source が static configuration、seed node list、AWS ECS、または generic adapter のいずれであっても、Cluster Discovery Provider Interop は同じ topology update contract を公開しなければならない。
2. topology input が provider boundary を越えた後、Cluster Discovery Provider Interop は backend name、cloud metadata、polling state を cluster core の placement decision に渡してはならない。
3. block list provider が authority を blocked として返す場合、Cluster Discovery Provider Interop は既存 topology update の blocked member contract を維持しなければならない。
4. downstream membership spec が reachability や WeaklyUp を要求する場合、Cluster Discovery Provider Interop はそれらを discovery adapter の責務として実装してはならない。

### 要件5: roadmap scope protection

**目的:** cluster roadmap の実装者として、この仕様が provider/discovery interop の範囲に留まり、隣接 spec の実装責務を吸収しないことを確認したい。

#### 受け入れ条件

1. gossip heartbeat または full Gossip merge が必要な場合、Cluster Discovery Provider Interop は `cluster-gossip-heartbeat-protocol` の責務として残さなければならない。
2. downing / SBR decision が必要な場合、Cluster Discovery Provider Interop は `cluster-downing-sbr-decision-model` の責務として残さなければならない。
3. pubsub mediator または topic registry gossip が必要な場合、Cluster Discovery Provider Interop は `cluster-pubsub-mediator-protocol` の責務として残さなければならない。
4. cluster message serializer が必要な場合、Cluster Discovery Provider Interop は `cluster-message-serialization-contract` の責務として残さなければならない。
5. gap analysis を更新するとき、Cluster Discovery Provider Interop は `SeedNodeProcess` と generic discovery adapter の evidence だけを完了候補として扱い、Deferred Pekko concepts を完了対象に含めてはならない。
