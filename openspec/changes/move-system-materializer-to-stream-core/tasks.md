## 1. Core Materialization 公開面

- [ ] 1.1 `SystemMaterializer` を `modules/stream-core-kernel/src/materialization/` 配下に追加し、`std::vec::Vec` を `alloc::vec::Vec` に置き換える。
- [ ] 1.2 `SystemMaterializerId` を `modules/stream-core-kernel/src/materialization/` 配下に追加し、actor-core の `ExtensionId` contract を維持する。
- [ ] 1.3 両方の型を `stream-core-kernel::materialization` から公開する。
- [ ] 1.4 rustdoc link を core materialization path に更新する。

## 2. Std アダプタ境界

- [ ] 2.1 `stream-adaptor-std` の materializer module と、旧 `SystemMaterializer` / `SystemMaterializerId` 定義を削除する。
- [ ] 2.2 `stream-adaptor-std` の IO アダプタ（`FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、`SourceFactory`）は import 影響以外を変更しない。
- [ ] 2.3 `stream-adaptor-std` からの互換 re-export は追加しない。

## 3. テスト

- [ ] 3.1 既存の `SystemMaterializer` 振る舞い test を、sibling `*_test.rs` test として `stream-core-kernel` へ移す。
- [ ] 3.2 `fraktor_stream_core_kernel_rs::materialization::{SystemMaterializer, SystemMaterializerId}` の core 公開 API test を追加または更新する。
- [ ] 3.3 `stream-adaptor-std` package-boundary test を更新し、std アダプタ export だけを確認する。

## 4. 検証

- [ ] 4.1 `cargo test -p fraktor-stream-core-kernel-rs system_materializer` を実行する。
- [ ] 4.2 `cargo test -p fraktor-stream-adaptor-std-rs package_boundaries` を実行する。
- [ ] 4.3 `cargo check -p fraktor-stream-core-kernel-rs --no-default-features` を実行する。
- [ ] 4.4 `cargo fmt --check --all` を実行する。
