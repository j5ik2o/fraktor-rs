# StreamRef local handoff 実装計画

## 目的
Pekko `StreamRefs.sourceRef()` / `StreamRefs.sinkRef()` の契約意図を、fraktor-rs の `SourceRef` / `SinkRef` / `StreamRefs` public API として実装する。

## 実装順序
1. `SourceLogic` / `SinkLogic` に `StreamRefSettings` 注入 hook を追加し、`ActorMaterializerConfig` から実行時 logic へ渡す。
2. `SourceRef<T>` / `SinkRef<T>` と `StreamRefs` factory を 1 型 1 ファイルで追加する。
3. `core/impl/streamref` に internal protocol と local handoff runtime logic を追加する。
4. 既存 write_tests フェーズの public integration test と新規 unit test を通す。
5. clippy、対象テスト、`./scripts/ci-check.sh ai dylint` を実行する。

## スコープ外
- `StreamRefResolver`
- serializer / wire format
- remote transport
- TCP / TLS stream
