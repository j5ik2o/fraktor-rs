## 背景

`stream-core-kernel` は no_std の stream runtime crate である。`ActorMaterializer`、`ActorMaterializerConfig`、`Materializer`、snapshot support などの materialization contract を所有している。

`stream-adaptor-std` は std adapter crate である。現在の IO surface である `FileIO`、`StreamConverters`、`StreamInputStream`、`StreamOutputStream` は `std::fs`、`std::io`、std channel に依存している。一方で `SystemMaterializer` は `ActorMaterializer` を wrap し、actor-core の extension contract を実装する型であり、性質が異なる。現在の std 依存は `std::vec::Vec` だけであり、これは `alloc::vec::Vec` に置き換えられる。

## 目標 / 対象外

**目標:**

- `SystemMaterializer` と `SystemMaterializerId` を core materialization API にする。
- crate 依存方向は `stream-adaptor-std -> stream-core-kernel` のまま維持する。
- 実行時の意味論を DIP に合わせる。core logic は core contract に依存し、std adapter crate はプラットフォーム依存実装を提供する。
- 誤解を招く std materializer public module を削除する。
- `stream-core-kernel` の no_std 互換性を維持する。

**対象外:**

- この change では新しい materializer port trait を追加しない。
- `ActorMaterializer`、actor-system extension storage、scheduler、tick driver API は再設計しない。
- `stream-adaptor-std` に互換 re-export を残さない。
- `FileIO` や `StreamConverters` を std adapter crate の外へ移動しない。

## 判断

### 判断 1: 型を `stream-core-kernel::materialization` へ移す

`SystemMaterializer` と `SystemMaterializerId` は host adapter ではなく materialization runtime contract の一部なので、`ActorMaterializer` と同じ場所に置く。これにより公開パスと概念が一致する。

```text
fraktor_stream_core_kernel_rs::materialization::{SystemMaterializer, SystemMaterializerId}
```

代替案として `stream-adaptor-std` に re-export を残す案もあるが、採用しない。この project は pre-release であり、互換 shim より clean contract を優先する。

### 判断 2: プラットフォーム依存部分はこの移設に含めない

既存の `SystemMaterializerId::create_extension` は、すでに作成済みの `ActorSystem` から `ActorMaterializer` を構築する。これは std 実行を所有しない。プラットフォーム固有の scheduler / tick の振る舞いは、actor system configuration と std driver 実装に紐づいたままにする。

代替案として新しい materializer port trait を今追加する案もあるが、採用しない。現時点の `SystemMaterializer` には分離すべき std-only implementation point がなく、port 追加は投機的になる。

### 判断 3: core では `std` ではなく `alloc` を使う

`SystemMaterializer::stream_snapshots` は `Vec<StreamSnapshot>` を返す。core ではこれを `alloc::vec::Vec` として扱う。実装は `std::*` を import してはならず、default feature を追加してはならず、`cfg_std_forbid` を満たし続けなければならない。

### 判断 4: test で振る舞いと package 境界の両方を固定する

既存の振る舞い test は型と一緒に `stream-core-kernel` へ移す。公開 API test は新しい外部 import path を確認する。std package-boundary test は materializer 型の import をやめ、std adapter export だけを確認する。

## リスク / トレードオフ

- **import path が破壊的に変わる** → この破壊的変更を受け入れ、proposal に明記する。互換 shim は置かない。
- **core に std import が混入する** → `alloc::vec::Vec` を使い、no-default-features check と既存の cfg-std-forbid lint coverage で確認する。
- **古い stream package wording と spec がずれる** → 既存の `stream-package-structure` requirement を変更し、archive 時に古い std materializer contract が置き換わるようにする。
- **test が std-only test helper に依存する** → その依存は `stream-core-kernel` の dev-dependencies に閉じ込め、production code は no_std のまま維持する。
