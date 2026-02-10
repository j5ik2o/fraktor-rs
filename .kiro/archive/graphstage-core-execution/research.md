# リサーチログ: graphstage-core-execution

## Summary
- 現行の `modules/streams/src/core` では Source/Flow/Sink が `SourceLogic`/`FlowLogic`/`SinkLogic` を直接駆動しており、`GraphStage` は実行経路に接続されていない。
- `GraphInterpreter` は線形パイプライン（Source → Flow* → Sink）前提で、GraphStage ベースのイベント駆動に移行するための統合点が不足している。
- `ActorMaterializer` と `StreamDriveActor` はストリーム駆動の責務を担っており、GraphStage 中心化後も駆動の境界として維持するのが低リスクである。

## Research Log

### 現行の実行経路
- 対象: `modules/streams/src/core/source.rs`, `flow.rs`, `sink.rs`, `graph_interpreter.rs`
  - Source/Flow/Sink はそれぞれ `SourceLogic`/`FlowLogic`/`SinkLogic` を生成して実行する。
  - `GraphInterpreter` は `SourceDefinition`/`FlowDefinition`/`SinkDefinition` を用いて直列処理を行う。
- 示唆:
  - GraphStage を中心に据えるには、定義と実行の双方を GraphStage に統合する必要がある。

### GraphStage 周辺の現状
- 対象: `modules/streams/src/core/graph_stage.rs`, `graph_stage_logic.rs`, `stage_context.rs`
  - GraphStage は `GraphStageLogic` の生成器として定義されているが、実行系に接続されていない。
- 示唆:
  - GraphStage のフック（on_start/on_pull/on_push 等）を実行系の正規ルートにする必要がある。

### 駆動責務の既存パターン
- 対象: `modules/streams/src/core/actor_materializer.rs`, `stream_drive_actor.rs`
  - `StreamDriveActor` が `StreamHandle` を tick 駆動する構造を持つ。
- 示唆:
  - GraphStage 中心化後も、drive loop の外縁は維持し、実行内部のみ置き換えるのが適切。

## Architecture Pattern Evaluation
- 既存の Source/Flow/Sink DSL を維持しつつ、内部実行は GraphStage へ統一する方針が最も影響範囲を限定できる。
- no_std を維持し、std 側は `ActorMaterializer` で駆動する二層構成は継続するべきである。

## Decisions (Draft)
- GraphStage を唯一の実行抽象とし、`SourceLogic`/`FlowLogic`/`SinkLogic` への依存を廃止する。
- `GraphInterpreter` は GraphStageLogic のイベント駆動に合わせて構成を改める。
- 駆動境界は `StreamHandle` と `StreamDriveActor` に置き、外部イベントは再駆動契機として扱う。

## Risks
- 直列処理前提の実行系からイベント駆動へ移行するため、テストの期待値や挙動が変わる可能性がある。
- GraphStage 中心化に伴い、既存の Source/Flow/Sink 実装が大きく再構成される。

## Open Questions
- GraphStage の Pending 相当をどの層で表現するか。
- 外部イベント連携の再駆動方式をどの粒度で提供するか。

## Supporting References
- `modules/streams/src/core` の既存実装
