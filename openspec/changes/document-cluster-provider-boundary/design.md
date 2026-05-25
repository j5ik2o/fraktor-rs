## 背景

`cluster-core` は `ClusterProvider` trait、`LocalClusterProvider`、`StaticClusterProvider` を持ち、provider から `ClusterEvent::TopologyUpdated` を EventStream へ publish する。`cluster-adaptor-std` は remoting lifecycle event を local provider に接続する helper と、AWS ECS task discovery を topology update に変換する provider を持つ。

Grain runtime の operational contract は、provider / failure detector / downing を topology update または member departure の入力境界として扱う。次の downing / reachability 作業に進む前に、provider-specific discovery と core-owned topology semantics の境界を固定する。

## 目的 / 対象外

**目的:**

- local / static / AWS ECS provider が供給する membership input の境界を仕様化する。
- `cluster-core` が discovery backend、remoting subscription、AWS polling を知らないことを明記する。
- `cluster-core` が定義する provider port を std adapter が実装するときに保持すべき subscription / poller lifetime を boundary contract として整理する。
- 既存 provider tests を boundary contract に対応づけ、不足があれば小さい test / docs を追加する。

**対象外:**

- `DowningProvider` の decision API 変更。
- Split Brain Resolver、reachability matrix、failure detector policy の導入。
- provider discovery backend の追加。
- rebalance、remembered entities、in-flight drain の実装。
- Pekko Cluster API parity。

## 決定事項

### Decision 1: provider boundary は新 capability として切る

`cluster-grain-runtime-operational-contract` は Grain identity / cache / passivation の contract を定義する。provider boundary はその上流入力の責務なので、新しい `cluster-provider-boundary` capability として独立させる。

代替案: 既存 Grain runtime spec に provider requirement を追加する。これは一見小さいが、runtime core と provider lifecycle の責務が混ざり、後続の downing / reachability change が肥大化しやすい。

### Decision 2: core が port と policy を所有し、std は adapter 実装に留める

`cluster-core` の contract は、provider port、extension lifecycle、topology application policy を所有する。std 側はその port の adapter 実装として、remoting lifecycle や AWS ECS polling を topology input へ変換する。core の identity / placement は provider 種別を分岐しない。

代替案: std adapter が core runtime を直接駆動する前提で lifecycle を設計する。これは DIP と port-and-adapter の向きが逆になり、core policy が adapter の都合に引きずられる。

### Decision 3: lifetime は ownership boundary として扱う

remoting lifecycle subscription は、core-defined provider port へ外部 lifecycle signal を供給する adapter lifetime である。返された `EventStreamSubscription` の保持期間だけ有効であり、subscription が provider を強参照し続けてはならない。AWS ECS provider は start 時に poller lifetime を開始し、shutdown で停止 signal を出す境界を持つ。

代替案: subscription / poller を cluster extension が暗黙保持する前提にする。これは caller が何を保持すべきか見えにくく、shutdown 後の topology update の扱いも曖昧になる。

### Decision 4: downing decision は次 change に分離する

本 change は provider が member departure input を供給できる境界を固定するだけに留める。failure observation から down decision を作る model は、reachability 表現と一緒に別 change で扱う。

代替案: `DowningProvider` をこの change で decision contract に拡張する。API 変更が必要になり、provider boundary の文書化という小さい作業から外れる。

## リスク / トレードオフ

- [リスク] 文書化だけで実装の抜けを見逃す -> 緩和策: local / static / AWS ECS の既存 tests と spec scenario を対応させ、不足分だけ targeted test を追加する。
- [リスク] lifecycle boundary が provider 実装ごとに不揃いになる -> 緩和策: common requirement は EventStream / topology input に寄せ、provider-specific behavior は scenario で分ける。
- [リスク] downing と provider boundary が混同される -> 緩和策: spec と docs で downing decision model を明示的に非対象にする。
