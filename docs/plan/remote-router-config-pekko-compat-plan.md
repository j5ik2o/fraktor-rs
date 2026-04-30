# RemoteRouterConfig の Pekko 互換 API 形状への修正計画

## 概要

`RemoteRouterConfig<P>` を公開 API から撤去し、Pekko と同じ `RemoteRouterConfig(local, nodes)` の認知モデルへ寄せる。Rust 内部では `RemoteRouterPool` と `RemoteRoutingLogic` で pool / logic の差を隠蔽し、ユーザーや serializer binding に `RemoteRouterConfig<RoundRobinPool>` のような型引数を露出させない。

## 実装方針

- `RemoteRouterConfig<P: Pool>` を非ジェネリックな `RemoteRouterConfig` に置き換える。
- `RemoteRouterConfig::new(local, nodes)` は `local: impl Into<RemoteRouterPool>` を受ける。
- `RemoteRouterPool` enum で `RoundRobinPool`、`SmallestMailboxPool`、`RandomPool`、`ConsistentHashingPool` を保持する。
- `RemoteRoutingLogic` enum で各 routing logic を包み、`RoutingLogic` を委譲実装する。
- `RemoteRouterConfig` は `RouterConfig<Logic = RemoteRoutingLogic>` と `Pool` を実装し、Pekko と同じく local pool に責務を委譲する。

## Serialization / Expansion

- serializer binding は `RemoteRouterConfig` 1種類に統一する。
- wire format の pool tag は維持し、decode 結果は常に非ジェネリック `RemoteRouterConfig` にする。
- `RoundRobinPool`、`SmallestMailboxPool`、`RandomPool` は既存と同等に serialize / deserialize する。
- `ConsistentHashingPool` は runtime expansion では扱うが、任意クロージャの serialization は今回の範囲外として明示的に `NotSerializable` を返す。
- `RemoteRouteeExpansion` は非ジェネリック化し、戻り値を `Router<RemoteRoutingLogic>` にする。

## テスト方針

- 型引数なしの `RemoteRouterConfig::new(RoundRobinPool::new(...), nodes)` が使えることを確認する。
- `Pool` / `RouterConfig` の委譲が既存 pool で維持されることを確認する。
- serializer default binding が `RemoteRouterConfig` 1件だけになることを確認する。
- misc serializer の roundtrip が `RoundRobinPool` / `SmallestMailboxPool` / `RandomPool` で非ジェネリック `RemoteRouterConfig` として復元されることを確認する。
- `ConsistentHashingPool` を含む `RemoteRouterConfig` は serializer で明示的に失敗することを確認する。
- remote routee expansion が型引数なしで routee を生成できることを確認する。

## 前提

- 後方互換は不要とし、`RemoteRouterConfig<P>` 参照は破壊的に置き換える。
- consistent hashing の任意 hash mapper serialization は別課題にする。
- ソース変更後は最終的に `./scripts/ci-check.sh ai all` を実行する。
