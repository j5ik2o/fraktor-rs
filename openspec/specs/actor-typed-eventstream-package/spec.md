## ADDED Requirements

### Requirement: typed/eventstream/ package が新設される
`modules/actor/src/core/typed/eventstream/` を新設し、Pekko `typed/eventstream/EventStream.scala` に対応する package 境界を確立しなければならない。`EventStream` 型を配置し、`core/typed.rs` から `pub mod eventstream;` として公開する。

#### Scenario: eventstream package が新設される
- **WHEN** `modules/actor/src/core/typed/eventstream/` を確認する
- **THEN** ディレクトリと `typed/eventstream.rs` モジュール宣言ファイルが存在する
- **AND** `typed.rs` に `pub mod eventstream;` の宣言がある

#### Scenario: EventStream 型が eventstream package に配置される
- **WHEN** `modules/actor/src/core/typed/eventstream/` の内容を確認する
- **THEN** `event_stream.rs` が存在し `EventStream` 型が定義されている
