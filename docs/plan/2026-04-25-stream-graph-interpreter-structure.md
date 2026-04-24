# Stream GraphInterpreter 構造分割計画

## 目的
`GraphInterpreter` に集中している plan compile、edge buffer dispatch、snapshot 生成、failure disposition の責務を内部モジュールへ分割する。Pekko の interpreter / connection / snapshot 責務分離意図を、fraktor-rs の tick-based core に合わせて再表現する。

## 対象
- `modules/stream-core/src/core/impl/interpreter.rs`
- `modules/stream-core/src/core/impl/interpreter/graph_interpreter.rs`
- `modules/stream-core/src/core/impl/interpreter/` 配下の新規 internal module
- `modules/stream-core/src/core/impl/interpreter/graph_interpreter/tests.rs`
- `docs/gap-analysis/stream-gap-analysis.md`

## 実装手順
1. `CompiledGraphPlan` を追加し、`StreamPlan` から stage / edge / dispatch / index 情報を組み立てる責務を切り出す。
2. `BufferedEdge` を追加し、edge の port、materialized combine、buffer lifecycle、connection state 判定を保持する。
3. `OutletDispatchState` と `GraphConnections` を追加し、outlet から edge への round-robin dispatch、incoming poll、close / clear 操作を集約する。
4. `InterpreterSnapshotBuilder` を追加し、stage / edge から `RunningInterpreter` snapshot を組み立てる。
5. `FailureDisposition` を独立 enum に移し、terminal failure payload を保持する。
6. `GraphInterpreter` は既存 public/internal API を変えず、上記 internal module へ委譲する。
7. `docs/gap-analysis/stream-gap-analysis.md` の GraphInterpreter 構造ギャップを更新する。

## スコープ外
- drive state machine 本体の大規模分割
- StreamRef remote resolver / serializer
- TCP / TLS stream
- `default_operator_catalog.rs` 分割
