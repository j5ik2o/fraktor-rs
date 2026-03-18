## Why

`modules/actor/src/std` には、`core` の port を実装する adapter と、`core` 型をそのまま包み直した façade / wrapper が混在している。現在の構成では `std` の責務境界が曖昧で、公開面の維持コストと依存追跡コストが高い。

後方互換は不要な開発フェーズなので、今のうちに `std` を「`core` の port に対する std/tokio/tracing adapter 実装」へ寄せ、意味の薄い wrapper を縮退させる。

## What Changes

- `modules/actor/src/std` の公開面を見直し、`core` mirror になっている純粋 wrapper を公開 API から外す
- **BREAKING** `std::typed::actor` 配下の `TypedActorContext` / `TypedActorContextRef` / `TypedActorRef` / `TypedChildRef` を外部公開対象から外す
- **BREAKING** `std::typed::TypedProps` / `std::typed::TypedActorSystem` を外部公開対象から外す
- **BREAKING** `std::actor::ActorContext` / `std::props::Props` / `std::system::ActorSystemConfig` を外部公開対象から外す
- `examples` / `tests` / `std` 内部実装の依存先を `std` façade から `core` へ付け替える
- façade 依存が消えた後に、`ActorAdapter` / `TypedActorAdapter` のような shim を削除する
- `dispatch` / `scheduler` / `event` の adapter 実装は維持し、`std` の中核として残す

## Capabilities

### New Capabilities
- `actor-std-adapter-surface`: `modules/actor/src/std` の公開面を adapter 実装中心に整理し、純粋 wrapper を公開 API から除外する

### Modified Capabilities

なし

## Impact

- 影響コード: `modules/actor/src/std`, `modules/actor/examples`, `modules/actor/src/std/tests.rs`, 関連する core typed / untyped import
- 影響 API: `crate::std::actor`, `crate::std::props`, `crate::std::typed`, `crate::std::system` の一部公開型
- 影響範囲: examples とテストの import 付け替え、`std.rs` の再エクスポート整理、wrapper/shim 削除
