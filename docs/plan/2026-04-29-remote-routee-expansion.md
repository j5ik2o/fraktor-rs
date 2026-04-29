# RemoteRouterConfig routee 展開実装計画

## 目的

remote Phase 2 の残対象である `RemoteRouterConfig` runtime routee expansion を実装する。

## 対象

- `modules/remote-adaptor-std/src/std/provider/remote_routee_expansion.rs`
- `modules/remote-adaptor-std/src/std/provider/remote_routee_expansion_error.rs`
- `modules/remote-adaptor-std/src/std/provider.rs`
- `modules/remote-adaptor-std/src/std/provider/tests.rs`

## 方針

- actor-core の `RemoteRouterConfig` は no_std 設定型のまま維持する。
- std 側 adapter が `RemoteRouterConfig<P>` と path factory を受け取り、`P::nr_of_instances()` 分の remote actor path を `StdRemoteActorRefProvider` で解決する。
- 解決した `ActorRef` は `Routee::ActorRef` として `P::create_router()` の router に注入する。
- routee path 生成失敗と provider 解決失敗は index と path を含む `Result` で返す。

## スコープ外

- remote child actor deployment daemon
- payload serialization
- inbound envelope delivery
- remote DeathWatch / watcher effects
- Pekko wire byte compatibility
