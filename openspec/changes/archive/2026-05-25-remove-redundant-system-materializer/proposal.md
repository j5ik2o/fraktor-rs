## 背景

`SystemMaterializer` は現在 `stream-adaptor-std` から公開されているが、実体は `ActorMaterializer` を 1 フィールドで包む薄い wrapper である。通常の stream 実行経路では `ActorMaterializer::new(system, config)` が直接使われており、`SystemMaterializer` / `SystemMaterializerId` は自身の unit test と package-boundary test 以外で実質的に使われていない。

この状態で core へ移設すると、存在価値の薄い Pekko 由来の shell 型を core API として固定してしまう。現時点では、core へ移すのではなく削除し、明示的な `ActorMaterializer` 構築を正規経路として維持する。

## 変更内容

- **BREAKING**: `SystemMaterializer` と `SystemMaterializerId` を削除する。
- **BREAKING**: `fraktor_stream_adaptor_std_rs::materializer::*` を削除する。
- `SystemMaterializer` / `SystemMaterializerId` は `stream-core-kernel` に移設しない。
- 既存の明示的な `ActorMaterializer::new(system, config)` を stream materialization の正規経路として維持する。
- `FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream` などの std 固有 stream API は `stream-adaptor-std` に残す。
- `SystemMaterializer` 専用 test と std package-boundary test の materializer export 確認を削除する。
- 将来、本当に actor system ごとの default materializer が必要になった場合は、config / lifecycle / DSL 接続を含む別 change として設計する。

## Capabilities

### New Capabilities

- なし。

### Modified Capabilities

- `stream-package-structure`: std adapter package は std-backed adapter だけを公開し、冗長な `SystemMaterializer` shell 型を削除する。core materialization package も `SystemMaterializer` を新設せず、`ActorMaterializer` を正規 surface として維持する。

## 影響

- `fraktor_stream_adaptor_std_rs::materializer::*` は利用できなくなる。
- `SystemMaterializer` / `SystemMaterializerId` の利用側は、`ActorMaterializer::new(system, config)` を直接使う必要がある。
- `modules/stream-core-kernel` には新しい materializer wrapper 型を追加しない。
- `modules/stream-adaptor-std` は IO adapter 型だけを保持する。
