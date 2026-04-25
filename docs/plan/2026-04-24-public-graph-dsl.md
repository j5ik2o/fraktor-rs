# Public GraphDSL facade 実装計画

## 背景
Pekko 互換仕様の stream core public authoring API として、`GraphDSL.create` と `GraphDSL.Builder` 相当を Rust の明示的 builder API で公開する。Scala の `~>` 構文や Java DSL は Rust API の対象外とする。

## 実装対象
- `modules/stream-core/src/core/dsl/graph_dsl.rs` に public `GraphDsl` を追加する。
- `modules/stream-core/src/core/dsl/graph_dsl_builder.rs` に public `GraphDslBuilder<In, Out, Mat>` を追加する。
- 既存 `core/impl` の graph builder machinery は内部実装として維持し、public API から raw `StreamGraph` や `into_parts` を露出しない。
- `core/dsl.rs` から `GraphDsl` / `GraphDslBuilder` を公開する。
- internal GraphDSL facade の不要な `dead_code` 回避を排除し、public facade に利用者入口を移す。

## スコープ外
- `SubSinkInlet` / `SubSourceOutlet`
- StreamRef 本体、settings、protocol、serializer
- TCP/TLS
- StageActorRef
- Scala `~>` / implicit syntax
- Java DSL / Reactive Streams TCK

## 検証
- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy -p fraktor-stream-core-rs --lib -- -D warnings`
- `rtk cargo test -p fraktor-stream-core-rs --lib graph_dsl`
- `rtk cargo test -p fraktor-stream-core-rs --test graph_dsl_public`
- `rtk ./scripts/ci-check.sh ai dylint`
