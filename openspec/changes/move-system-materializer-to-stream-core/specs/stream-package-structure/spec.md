## MODIFIED Requirements

### Requirement: std adapter は io と materializer の責務境界で整理されなければならない
`modules/stream-adaptor-std` は、std 環境に固有の stream adapter だけを公開しなければならない（MUST）。`FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、std-backed source adapter は `io` package 境界に集約される。`SystemMaterializer` と `SystemMaterializerId` は `modules/stream-core-kernel/src/materialization` に属し、std adapter crate はこれらを定義または互換 re-export してはならない（MUST NOT）。

#### Scenario: std の IO adapter が io package に集約される
- **WHEN** `modules/stream-adaptor-std` の IO 関連型を確認する
- **THEN** `FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、std-backed source adapter は `io` package 境界に配置される

#### Scenario: system materializer が core materialization package に集約される
- **WHEN** `SystemMaterializer` と `SystemMaterializerId` の配置を確認する
- **THEN** それらは `modules/stream-core-kernel/src/materialization` 配下に配置される
- **AND** 外部 caller は `fraktor_stream_core_kernel_rs::materialization::{SystemMaterializer, SystemMaterializerId}` から参照できる

#### Scenario: std adapter は system materializer を公開しない
- **WHEN** `fraktor_stream_adaptor_std_rs` の公開 surface を確認する
- **THEN** `materializer` public module は存在しない
- **AND** `fraktor_stream_adaptor_std_rs::materializer::SystemMaterializer` と `fraktor_stream_adaptor_std_rs::materializer::SystemMaterializerId` は公開されない

#### Scenario: core materializer extension は std に依存しない
- **WHEN** `modules/stream-core-kernel/src/materialization` 配下の `SystemMaterializer` と `SystemMaterializerId` 実装を確認する
- **THEN** `std::*` import と `extern crate std` は production code に存在しない
- **AND** `SystemMaterializer::stream_snapshots` は `alloc::vec::Vec<StreamSnapshot>` を返せる
