# lock-driver-port-adapter 失敗理由メモ

## 概要

この change は一旦破棄する。主因は、`RuntimeMutex` / `RuntimeRwLock` の driver 差し替えを導入する際に、設計上の責務境界と型の伝播境界を同時に満たせなかったためである。

## 何が問題だったか

### 1. `D` を型引数にしたことで設計が複雑化した

`RuntimeMutex<T, D>` / `RuntimeRwLock<T, D>` とすると、これを field に持つ型も `D` を意識する必要が出る。

結果として次のいずれかを迫られる。

- 型引数 `D` を `Mailbox` / `ActorCell` / `ActorSystem` まで伝播させる
- 途中で trait object / enum / erased wrapper に落とす
- build-time または runtime のどこかで concrete driver を強制固定する

この時点で、lock driver は単なる内部実装詳細ではなく、型設計全体へ影響する論点になった。

### 2. public API に `D` を漏らしたくない

今回の判断として、次は NG とした。

- `ActorSystem<D>`
- `ActorRef<D>`
- typed system の public generic 化

これは Pekko 互換の観点でも、lock driver が actor runtime の public concept ではないという観点でも不適切だった。

### 3. `core` から `std adaptor` へ依存できない

一時的に検討した「actor-core 側の private alias で `Spin / DebugSpin / Std` を build-time 切替する」案は、`core` が `utils-adaptor-std` の型を知る必要があり破綻した。

つまり次は成立しない。

```text
actor-core/core
  ActorCoreMutex<T> = DebugSpinSyncMutex<T> / StdSyncMutex<T>
```

これは core/std 分離に反する。

### 4. runtime 選択と static dispatch と nongeneric public API を同時に満たせなかった

現行 actor-core の構造では:

- `Dispatchers` registry は configurator を trait object で保持する
- `MessageDispatcherConfigurator::dispatcher()` は nongeneric `MessageDispatcherShared` を返す
- `SystemState::cell(pid)` は concrete `ArcShared<ActorCell>` を返す

このため、次の 3 条件を同時に満たす設計は成立しなかった。

- driver family を runtime / config で選ぶ
- static dispatch を維持する
- public API を nongeneric のまま保つ

## 途中で分かったこと

### 良かった判断

- `SpinSyncMutex` は no_std builtin として core 側に残す
- `DebugSpinSyncMutex` / `StdSyncMutex` は std adapter 側に置く
- poison は driver 実装側で吸収し、caller へ露出しない
- `Mutex` と `RwLock` は対称に設計する

### 破綻した判断

- hot path へ genericization を素直に通す
- actor-core `core/` 側で build-time selection する
- runtime/configurator 境界で driver family を選びつつ public API を nongeneric に保つ

## 学び

### 1. lock driver のような内部実装詳細は public 型引数にしてはいけない

`D` は public surface に現れた瞬間に abstraction leak になる。

### 2. trait / alias / factory を決める前に、依存方向を決めるべきだった

特に次を先に fix すべきだった。

- driver 実装はどの crate に置くか
- driver 選択はどの層で行うか
- 型引数伝播をどこで止めるか

### 3. この change は「実装詳細を書きすぎた」のではなく、「API 形状の核心を決めないまま進めようとした」ことが失敗だった

`mailbox-cleanup-ownership-handoff` のような invariant-heavy な change と違い、これは API-shape-heavy な change だった。

そのため、実装で埋めればよい余白と、設計で先に決めるべき核心の区別が必要だった。

## 次回やるなら

次に同種の変更を再提案する場合は、まず以下を 1 つに固定してから始める。

- driver selection を compile-time にするのか
- runtime selection にするのか
- `RuntimeMutex` 自体が driver を隠蔽するのか
- 上位型が facade として `D` を隠蔽するのか

特に次は最初に明記する。

- public API に `D` を漏らさない
- core は std adaptor 型を知らない
- driver selection の責務境界

## 結論

この change は「方針の一部は正しかった」が、「依存方向と型伝播境界を決める前に進めた」ため破綻した。

再挑戦するなら、まずは

- 依存方向
- driver selection のタイミング
- `D` の隠蔽戦略

の 3 点を先に固定すること。
