# 要件ドキュメント

## 導入

この仕様は、`docs/gap-analysis/cluster-gap-analysis.md` の `Active comparison follow-up: trivial / easy` に含まれる cluster 互換 surface を、後続 spec が参照できる baseline として固定する。対象は config compatibility、`remotePathOf` 相当の remote actor path helper、downing provider / SBR settings の compatibility metadata、transport lifecycle bridge retention に限定する。

この仕様は Pekko public API parity や full Split Brain Resolver 実装を目的にしない。membership reachability、gossip/heartbeat、downing decision、discovery provider、pubsub mediator、cluster message serialization は downstream spec の責務として残す。

## 要件

### 要件1: config compatibility baseline

**目的:** cluster runtime 実装者として、join compatibility で比較すべき cluster 設定 surface を安定した語彙で扱い、後続 membership / discovery / downing work が同じ compatibility contract を参照できるようにしたい。

#### 受け入れ条件

1. cluster node が join compatibility を評価するとき、Cluster Compatibility Baseline は比較対象キー、比較対象外キー、比較理由を観測可能な結果として返さなければならない。
2. downing provider または Split Brain Resolver settings が異なる場合、Cluster Compatibility Baseline は join を incompatible として報告しなければならない。
3. sensitive または local-only な設定が存在する場合、Cluster Compatibility Baseline はそれらを join compatibility の比較対象から除外しなければならない。
4. additional checker を含む場合、Cluster Compatibility Baseline は checker ごとの incompatible reason を失わずに合成しなければならない。
5. Cluster Compatibility Baseline は常に pubsub、downing provider、SBR settings、failure detector choice に関する compatibility 語彙を後続 spec から参照できる形で公開しなければならない。

### 要件2: remote actor path helper

**目的:** cluster API 利用者として、local actor ref と remote actor ref のどちらからでも cluster が公開できる canonical remote path を取得し、remote delivery や運用ログで同じ path 表記を使えるようにしたい。

#### 受け入れ条件

1. local actor ref の remote path が要求されたとき、Cluster API は local actor path を cluster advertised authority 付きの canonical remote path として返さなければならない。
2. actor ref がすでに remote authority を持つ場合、Cluster API は既存 authority と path segments を変更せずに返さなければならない。
3. actor ref に canonical path が存在しない場合、Cluster API は caller から観測可能な失敗として報告しなければならない。
4. actor ref に UID が含まれる場合、Cluster API は返却する remote path に UID を保持しなければならない。

### 要件3: downing provider compatibility baseline

**目的:** downing / SBR spec 実装者として、SBR decision model 本体を先に実装しなくても、provider key と SBR settings identity を互換性判定の語彙として参照できるようにしたい。

#### 受け入れ条件

1. cluster extension が downing provider compatibility を評価するとき、Cluster Compatibility Baseline は provider key と SBR settings identity を比較可能な metadata として公開しなければならない。
2. SBR settings identity が存在しない場合、Cluster Compatibility Baseline は no-op downing provider と custom provider factory の既存 behavior を変更してはならない。
3. Cluster Compatibility Baseline は downing decision hook、reachability matrix、lease majority 判定を実装したものとして振る舞ってはならない。
4. downstream `cluster-downing-sbr-decision-model` が provider-facing SBR integration を実装する場合、Cluster Compatibility Baseline は同じ metadata 語彙を再利用できる状態にしなければならない。

### 要件4: transport lifecycle bridge retention

**目的:** std adaptor 利用者として、remoting lifecycle subscription の lifetime が provider lifecycle と一致し、helper return 後も connected / quarantined events が membership input として届くことを期待したい。

#### 受け入れ条件

1. std adaptor が remoting lifecycle bridge を開始したとき、Cluster Compatibility Baseline は subscription guard を caller または provider state が保持できる形で返さなければならない。
2. subscription guard が保持されている間、connected event が起きたとき、std adaptor は started provider に topology join input を届けなければならない。
3. subscription guard が drop された場合、std adaptor は以後の remoting lifecycle events から topology update を生成してはならない。
4. provider への strong handle がすべて drop された場合、remoting lifecycle bridge は provider を生存させ続けてはならない。

### 要件5: downstream scope protection

**目的:** cluster roadmap の実装者として、この baseline が downstream spec の責務を吸収せず、後続作業の独立性を保てるようにしたい。

#### 受け入れ条件

1. membership reachability behavior が必要な場合、Cluster Compatibility Baseline は `cluster-membership-reachability-model` の責務として残さなければならない。
2. gossip、heartbeat、downing decision、discovery provider、pubsub mediator、message serialization が必要な場合、Cluster Compatibility Baseline は該当 downstream spec の責務として残さなければならない。
3. gap analysis を更新するとき、Cluster Compatibility Baseline は trivial / easy の4項目だけを完了候補として扱い、Deferred Pekko concepts を完了対象に含めてはならない。
