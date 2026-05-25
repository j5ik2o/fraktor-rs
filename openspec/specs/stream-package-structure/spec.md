# stream-package-structure Specification

## Purpose
stream package の DSL、GraphStage、runtime internals、std adapter、root 公開面の境界を定義する。
## Requirements
### Requirement: stream DSL は execution internals から独立した package 境界を持たなければならない
`modules/stream/src/core` は、Rust 利用者向けの stream DSL を execution internals や GraphStage helper と同居させてはならない。`Source`、`Flow`、`Sink`、`BidiFlow`、`FlowWithContext`、`SourceWithContext`、subflow 群、restart DSL 群は、Pekko の `scaladsl` / `javadsl` に対応する単一の Rust DSL package に MUST 集約される。

#### Scenario: DSL 型が単一 package に集約される
- **WHEN** `modules/stream/src/core` の DSL 公開型を確認する
- **THEN** `Source`、`Flow`、`Sink`、`BidiFlow`、`FlowWithContext`、`SourceWithContext`、`FlowSubFlow`、`SourceSubFlow` は `stage` 直下ではなく DSL package 配下に配置される

#### Scenario: stage package が DSL の入口ではなくなる
- **WHEN** `modules/stream/src/core/stage/` の公開面を確認する
- **THEN** `stage` package は `Source`、`Flow`、`Sink` の主要参照経路として使われない
- **AND** 利用側は DSL package 経由で stream 操作型を参照する

### Requirement: GraphStage 基盤は stage package に限定されなければならない
`modules/stream/src/core/stage` は、Pekko の `org.apache.pekko.stream.stage` に対応する GraphStage 基盤と stage helper に限定されなければならない。`GraphStage`、`GraphStageLogic`、timer / async callback helper、stage context、stage kind は `stage` package に MUST 残り、DSL surface や fusing internals を抱えてはならない。

#### Scenario: GraphStage 基盤が stage package に残る
- **WHEN** `modules/stream/src/core/stage/` を確認する
- **THEN** `GraphStage`、`GraphStageLogic`、`TimerGraphStageLogic`、`AsyncCallback`、`StageContext`、`StageKind` は `stage` package 配下に存在する

#### Scenario: stage package が internal fusing 実装の集積所にならない
- **WHEN** interpreter や flow logic 実装の配置先を確認する
- **THEN** `graph_interpreter` や `flow/logic/*` のような runtime internals は `stage` package 直下ではなく internal implementation package に配置される

### Requirement: runtime internals は Pekko `impl` / `impl/fusing` 対応の package に再編されなければならない
`modules/stream/src/core` の interpreter、boundary、traversal、graph wiring、flow logic、source logic、sink logic の内部実装は、Pekko の `org.apache.pekko.stream.impl` および `org.apache.pekko.stream.impl.fusing` に対応する package に MUST 集約される。公開 DSL と internal execution runtime は同じ package 階層に混在してはならない。

#### Scenario: interpreter と boundary 実装が impl package に集約される
- **WHEN** graph interpreter と island boundary 周辺の実装を確認する
- **THEN** `GraphInterpreter`、`BoundarySinkLogic`、`BoundarySourceLogic`、`IslandBoundary`、`IslandSplitter` は internal implementation package 配下に配置される

#### Scenario: fused operator logic が fusing package に集約される
- **WHEN** map / filter / merge / zip / timeout / conflate などの flow logic 実装を確認する
- **THEN** `stage/flow/logic/*` に存在していた fused operator logic は `impl/fusing` に対応する package 配下へ移される

### Requirement: std adapter は io と materializer の責務境界で整理されなければならない
`modules/stream-adaptor-std` は、std 環境に固有の stream adapter だけを公開しなければならない（MUST）。`FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、std-backed source adapter は `io` package 境界に集約される。`SystemMaterializer` と `SystemMaterializerId` は独自の lifecycle、config 注入、DSL 連携を持たない冗長な wrapper であるため削除されなければならない（MUST）。`stream-adaptor-std` はこれらを定義または互換 re-export してはならず（MUST NOT）、`stream-core-kernel` もこの change で代替 wrapper として追加してはならない（MUST NOT）。

#### Scenario: std の IO adapter が io package に集約される
- **WHEN** `modules/stream-adaptor-std` の IO 関連型を確認する
- **THEN** `FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、std-backed source adapter は `io` package 境界に配置される

#### Scenario: system materializer shell は削除される
- **WHEN** `SystemMaterializer` と `SystemMaterializerId` の公開型を確認する
- **THEN** `fraktor_stream_adaptor_std_rs::materializer::SystemMaterializer` と `fraktor_stream_adaptor_std_rs::materializer::SystemMaterializerId` は公開されない
- **AND** `fraktor_stream_core_kernel_rs::materialization::SystemMaterializer` と `fraktor_stream_core_kernel_rs::materialization::SystemMaterializerId` も公開されない

#### Scenario: std adapter は materializer module を公開しない
- **WHEN** `fraktor_stream_adaptor_std_rs` の公開面を確認する
- **THEN** `materializer` public module は存在しない

#### Scenario: 明示的な ActorMaterializer が正規経路として残る
- **WHEN** stream graph を実行する public test または example を確認する
- **THEN** caller は `ActorMaterializer::new(system, config)` または `ActorMaterializer` を返す helper を使う
- **AND** `SystemMaterializer` 経由の materialization 経路は存在しない

#### Scenario: default materializer はこの change で新設しない
- **WHEN** `modules/stream-core-kernel/src/materialization` の公開型を確認する
- **THEN** actor system ごとの default materializer を表す新しい wrapper 型は追加されていない
- **AND** default materializer が必要な場合は config、lifecycle、DSL 解決経路を含む別 change で扱う

### Requirement: root 公開面は root abstractions に限定されなければならない
`modules/stream/src/core.rs` の root 公開面は、Pekko root に相当する抽象型・設定型・shape・strategy・materializer などの root abstractions に限定されなければならない。DSL 型や internal implementation 型は root 直下へ広く再 export されず、対応する package 経由で MUST 参照される。

#### Scenario: root から DSL 型を広く再 export しない
- **WHEN** `modules/stream/src/core.rs` の `pub use` を確認する
- **THEN** `Source`、`Flow`、`Sink`、`BidiFlow` は root 直下の主要公開面として再 export されない

#### Scenario: root から internal implementation 型を公開しない
- **WHEN** `modules/stream/src/core.rs` と internal package の公開面を確認する
- **THEN** interpreter、boundary、fusing logic は root 直下の公開 API として露出しない
