## 1. 冗長な Materializer Shell の削除

- [ ] 1.1 `stream-adaptor-std` の `SystemMaterializer` 定義を削除する。
- [ ] 1.2 `stream-adaptor-std` の `SystemMaterializerId` 定義を削除する。
- [ ] 1.3 `stream-adaptor-std` の `materializer` public module を削除する。
- [ ] 1.4 `stream-core-kernel` には `SystemMaterializer` / `SystemMaterializerId` を追加しない。

## 2. Std アダプタ公開面の整理

- [ ] 2.1 `stream-adaptor-std` の IO アダプタ（`FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream`、`SourceFactory`）は import 影響以外を変更しない。
- [ ] 2.2 `stream-adaptor-std` からの互換 re-export は追加しない。
- [ ] 2.3 `ActorMaterializer::new(system, config)` を明示的な materialization 正規経路として残す。

## 3. テスト

- [ ] 3.1 `SystemMaterializer` 専用 unit test を削除する。
- [ ] 3.2 `stream-adaptor-std` package-boundary test を更新し、std アダプタ export だけを確認する。
- [ ] 3.3 `stream-core-kernel` に `SystemMaterializer` / `SystemMaterializerId` の公開 API test を追加しない。
- [ ] 3.4 既存の `ActorMaterializer` public / behavior tests が明示的 materializer 経路を引き続き固定していることを確認する。

## 4. 検証

- [ ] 4.1 `rg -n "SystemMaterializer|SystemMaterializerId" modules/stream-core-kernel modules/stream-adaptor-std` を実行し、実装側の残存参照がないことを確認する。
- [ ] 4.2 `cargo test -p fraktor-stream-adaptor-std-rs --test package_boundaries` を実行する。
- [ ] 4.3 `cargo test -p fraktor-stream-core-kernel-rs --lib` を実行する。
- [ ] 4.4 `cargo check -p fraktor-stream-core-kernel-rs --no-default-features` を実行する。
- [ ] 4.5 `cargo fmt --check --all` を実行する。
