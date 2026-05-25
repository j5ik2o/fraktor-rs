## MODIFIED Requirements

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
