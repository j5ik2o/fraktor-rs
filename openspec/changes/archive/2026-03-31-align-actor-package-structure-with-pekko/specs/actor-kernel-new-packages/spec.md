## ADDED Requirements

### Requirement: kernel/io/ / kernel/routing/ / kernel/util/ / kernel/actor/setup/ の package 境界が確立される
`modules/actor/src/core/kernel/` に `io/`・`routing/`・`util/`・`actor/setup/` を新設し、Pekko の `io.*`・`routing.*`・`util.*`・`actor.setup.*` に対応する package 境界を確立しなければならない。`util/` には `messaging/byte_string` を移設する。`io/` と `routing/` は今回 stub として新設し、将来の実装を受け入れる構造とする。

#### Scenario: kernel/util/ が新設され ByteString が移設される
- **WHEN** `modules/actor/src/core/kernel/util/` を確認する
- **THEN** `byte_string.rs` が `util/` 配下に存在する
- **AND** `crate::core::kernel::messaging::ByteString` の旧 import path は削除され、`crate::core::kernel::util::ByteString` が正しいパスになる

#### Scenario: kernel/io/ が stub として新設される
- **WHEN** `modules/actor/src/core/kernel/io/` を確認する
- **THEN** ディレクトリと `kernel/io.rs` モジュール宣言ファイルが存在する
- **AND** `kernel.rs` に `pub mod io;` の宣言がある

#### Scenario: kernel/routing/ が stub として新設される
- **WHEN** `modules/actor/src/core/kernel/routing/` を確認する
- **THEN** ディレクトリと `kernel/routing.rs` モジュール宣言ファイルが存在する
- **AND** `kernel.rs` に `pub mod routing;` の宣言がある

#### Scenario: kernel/actor/setup/ が新設される
- **WHEN** `modules/actor/src/core/kernel/actor/setup/` を確認する
- **THEN** ディレクトリと関連ファイルが存在する
- **AND** `actor.rs` または `actor/` に `pub mod setup;` の宣言がある
