## 背景

`SystemMaterializer` は現在 `stream-adaptor-std` から公開されているが、`std::io`、Tokio、ファイルシステム、ネットワークの adapter ではない。共有 `ActorMaterializer` を保持する actor-system extension であり、stream core の materialization 概念である。

これを std adapter crate に置くと、ports-and-adapters の境界が曖昧になる。core logic は core contract に依存し、std crate はその contract に対するプラットフォーム依存実装を提供する立場に限定する。

## 変更内容

- **BREAKING**: `SystemMaterializer` と `SystemMaterializerId` を `fraktor_stream_adaptor_std_rs::materializer` から `fraktor_stream_core_kernel_rs::materialization` へ移す。
- 互換 re-export は置かず、`stream-adaptor-std` の materializer public module を削除する。
- `FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream` などの std 固有 stream API は `stream-adaptor-std` に残す。
- `SystemMaterializer` の現在の `std::vec::Vec` 利用を `alloc::vec::Vec` に置き換え、no_std core crate で成立する型にする。
- `SystemMaterializerId` は、`ActorMaterializer::new(system.clone(), ActorMaterializerConfig::new())` を構築する actor-core `ExtensionId` として維持する。
- materializer の振る舞いと公開 API contract は core 側の test で固定し、std package-boundary test は std adapter export だけを確認する。

## Capabilities

### New Capabilities

- なし。

### Modified Capabilities

- `stream-package-structure`: system materializer の所有権を std adapter package から stream core materialization package へ移し、std adapter package は実際の std-backed adapter だけに狭める。

## 影響

- 公開 API の利用側は `SystemMaterializer` と `SystemMaterializerId` を `fraktor_stream_core_kernel_rs::materialization` から import する。
- `fraktor_stream_adaptor_std_rs::materializer::*` は削除される。
- `modules/stream-core-kernel` に materializer extension 型と test が追加される。
- `modules/stream-adaptor-std` は IO adapter 型だけを保持する。
- no_std 境界として、`stream-core-kernel` での `std::*` 利用禁止を維持する。
