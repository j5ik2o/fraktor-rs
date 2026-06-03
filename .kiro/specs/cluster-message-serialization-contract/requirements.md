# 要件ドキュメント

## 導入

この仕様は、cluster message の serializer contract を `fraktor-actor-core-kernel-rs` の既存 serialization subsystem と cluster std/wire 境界の間に定義する。対象は fraktor-rs cluster runtime 実装者、std adaptor 実装者、後続の interoperability 実装者であり、gossip envelope と pubsub mediator payload を同じ versioned serializer contract で扱える状態を作る。

この仕様は message serialization の接続点を扱うが、gossip merge semantics、pubsub mediator semantics、remote transport lifecycle、protobuf / Pekko wire protocol の完全互換は所有しない。

## 要件

### 要件1: actor-core serialization 接続
**目的:** cluster runtime 実装者として、cluster message を既存 actor-core serialization と同じ診断・登録経路で扱い、独自 codec の重複を避けたい。

#### 受け入れ条件
1. cluster message の送信準備が起きたとき、Cluster Message Serialization Contract は actor-core serialization の serializer id、manifest、payload bytes を保持した serialized message として表現しなければならない
2. serializer が登録されていない場合、Cluster Message Serialization Contract は cluster 固有の成功値へフォールバックせず、serialization failure として観測可能にしなければならない
3. actor-core serialization が manifest を返す場合、Cluster Message Serialization Contract はその manifest を wire bridge へ欠落なく渡さなければならない
4. actor-core serialization が manifest を返さない場合、Cluster Message Serialization Contract は payload kind と serializer id だけで roundtrip 可能な message に限定しなければならない

### 要件2: cluster message manifest と payload kind
**目的:** cluster protocol 実装者として、gossip payload と pubsub payload を wire 上で区別し、受信側が未知 payload を安全に拒否できるようにしたい。

#### 受け入れ条件
1. gossip message または pubsub message が cluster wire へ渡されるとき、Cluster Message Serialization Contract は cluster payload kind を明示しなければならない
2. payload kind が gossip の場合、Cluster Message Serialization Contract は gossip payload の意味論を評価せず、upstream `cluster-gossip-heartbeat-protocol` が定義した payload contract だけを参照しなければならない
3. payload kind が pubsub の場合、Cluster Message Serialization Contract は pubsub delivery や registry merge を実行せず、upstream `cluster-pubsub-mediator-protocol` が定義した payload contract だけを参照しなければならない
4. payload kind が未知の場合、Cluster Message Serialization Contract は受信 payload を actor message に復元してはならない
5. actor-core deserializer または manifest route が manifest 文字列を解決できない場合、Cluster Message Serialization Contract は cluster 固有の fallback に変換せず、serialization / decode failure として観測可能にしなければならない

### 要件3: std/wire bridge
**目的:** std adaptor 実装者として、cluster serialized message を versioned wire frame に変換し、transport lifecycle と message semantics を混ぜずに送受信したい。

#### 受け入れ条件
1. cluster serialized message が std/wire に渡されたとき、Cluster Message Serialization Contract は frame version、payload kind、serializer id、manifest、payload length、payload bytes を含む wire frame として表現しなければならない
2. wire frame が decode されたとき、Cluster Message Serialization Contract は actor-core serialization へ渡せる serialized message と cluster payload kind を復元しなければならない
3. payload length が実 bytes と一致しない場合、Cluster Message Serialization Contract は malformed payload として拒否しなければならない
4. std/wire bridge が失敗した場合、Cluster Message Serialization Contract は remote transport connection の開始・停止・再接続を実行してはならない

### 要件4: versioning と未知 payload handling
**目的:** interoperability 実装者として、将来の wire 変更と未知 payload を明確に検出し、silent corruption を避けたい。

#### 受け入れ条件
1. unsupported frame version を受信した場合、Cluster Message Serialization Contract は unknown version failure として観測可能にしなければならない
2. supported frame version だが unknown payload kind を受信した場合、Cluster Message Serialization Contract は unknown payload failure として観測可能にしなければならない
3. supported payload kind だが serializer id が解決できない場合、Cluster Message Serialization Contract は actor-core serialization の not-serializable / serializer lookup failure として扱わなければならない
4. decode failure が起きた場合、Cluster Message Serialization Contract は payload bytes を既定型・空 message・dead-letter 用 message に変換してはならない
5. versioned frame の schema が変わる場合、Cluster Message Serialization Contract は downstream specs に revalidation が必要な変更として扱わなければならない

### 要件5: scope boundary
**目的:** reviewer と実装者として、serialization contract の責務を狭く保ち、protocol semantics や binary compatibility の作業を吸収しないようにしたい。

#### 受け入れ条件
1. gossip payload が decode されたとき、Cluster Message Serialization Contract は gossip merge、seen digest、heartbeat evidence、reachability update を実行してはならない
2. pubsub payload が decode されたとき、Cluster Message Serialization Contract は mediator command application、delivery target selection、registry delta application を実行してはならない
3. std/wire bridge を含む場合、Cluster Message Serialization Contract は Tokio task lifecycle、socket ownership、remote association lifecycle を定義してはならない
4. Pekko / protobuf compatibility が要求された場合、Cluster Message Serialization Contract は完全な binary compatibility を現在 scope 外として明示しなければならない
5. actor-core serialization の public contract が変わる場合、Cluster Message Serialization Contract は actor-core serialization 全体の再設計ではなく、cluster 接続点の再検証として扱わなければならない
