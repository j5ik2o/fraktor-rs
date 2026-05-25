## 背景

`stream-core-kernel` は no_std の stream runtime crate であり、`ActorMaterializer`、`ActorMaterializerConfig`、`Materializer`、snapshot support などの materialization contract を所有している。現在の stream test や public surface test でも、stream 実行はほぼ `ActorMaterializer::new(system, config)` を直接使っている。

`SystemMaterializer` は `stream-adaptor-std` に置かれているが、std 固有の adapter ではない。一方で、core に移すほどの責務もまだ持っていない。`materializer_mut()` で中の `ActorMaterializer` をそのまま公開し、`stream_snapshots()` は `MaterializerState::stream_snapshots` へ委譲するだけである。`SystemMaterializerId` も `ActorMaterializerConfig::new()` 固定で extension を作るため、利用側が config を注入できない。

## 目標 / 対象外

**目標:**

- `SystemMaterializer` と `SystemMaterializerId` を削除する。
- `stream-adaptor-std` の公開面を std-backed adapter に限定する。
- `stream-core-kernel` には冗長な wrapper 型を追加しない。
- `ActorMaterializer::new(system, config)` を明示的な materialization 正規経路として維持する。
- 将来 default materializer を作る場合の判断基準を明確にする。

**対象外:**

- actor system ごとの default materializer を新設しない。
- `ActorMaterializer`、scheduler、tick driver、actor-system extension storage は再設計しない。
- `stream-adaptor-std` に互換 re-export を残さない。
- `FileIO` や `StreamConverters` を std adapter crate の外へ移動しない。

## 判断

### 判断 1: `SystemMaterializer` は移設ではなく削除する

`SystemMaterializer` は `ActorMaterializer` の所有権を包むだけで、独自の lifecycle、config、DSL 連携、不変条件を持っていない。さらに `materializer_mut()` で内部 materializer をそのまま可変公開するため、wrapper としての抽象化にもなっていない。

core へ移すと、この薄い wrapper を core public API として固定してしまう。したがって移設ではなく削除する。

### 判断 2: `SystemMaterializerId` も削除する

`SystemMaterializerId` は actor-core の `ExtensionId` contract を使っているが、`ActorMaterializerConfig::new()` 固定で `ActorMaterializer` を作るだけである。利用側が drive interval や stream ref settings などの config を注入できず、実用的な system default materializer になっていない。

ActorSystem extension として成立させるなら、config source、lifecycle、shutdown、DSL 側の既定 materializer 解決まで含める必要がある。それはこの change の範囲ではなく、将来の別 change とする。

### 判断 3: 明示的な `ActorMaterializer` を正規経路にする

現状の stream 実行では、caller が `ActorMaterializer::new(system, config)` を明示的に作り、graph 実行時に渡す形が一貫している。この経路は config が明示的で、余計な actor-system extension 登録も不要である。

したがって、この change 後も `ActorMaterializer` / `ActorMaterializerConfig` / `Materializer` が core materialization surface の中心であり続ける。

### 判断 4: default materializer は必要になった時点で再設計する

Pekko の `SystemMaterializer` 相当を将来導入するなら、単なる wrapper ではなく以下を満たす必要がある。

- actor system setup から materializer config を注入できる。
- actor system lifecycle / coordinated shutdown と連動する。
- stream DSL が default materializer を解決する明確な entry point を持つ。
- 複数回登録や再取得時の共有 semantics が test で固定される。

これらが揃うまでは `SystemMaterializer` という名前だけを残さない。

## リスク / トレードオフ

- **import path が破壊的に消える** → pre-release 方針に従い、互換 shim を置かず削除する。利用側は `ActorMaterializer::new(system, config)` に移行する。
- **Pekko parity の名前が減る** → 名前だけの parity より、実際の semantics が揃った時点で導入する方を優先する。
- **将来 default materializer が必要になる** → その時点で config / lifecycle / DSL 接続込みの別 change を作る。
