# Pekko → Rust 移植ナレッジ

## 参照実装の場所

| 実装 | パス | 言語 |
|------|------|------|
| Apache Pekko (actor) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/` | Scala |
| Apache Pekko (actor-typed) | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` | Scala |
| Apache Pekko (streams) | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` | Scala |
| Apache Pekko (cluster) | `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/` | Scala |
| Apache Pekko (remote) | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` | Scala |
| protoactor-go | `references/protoactor-go/` | Go |

## fraktor-rs モジュール構造

```text
modules/
├── utils/       # fraktor-utils-rs: 共有ユーティリティ
├── actor/       # fraktor-actor-rs: アクターシステムコア
├── remote/      # fraktor-remote-rs: リモーティング
├── cluster/     # fraktor-cluster-rs: クラスタリング
└── streams/     # fraktor-streams-rs: ストリーム処理
```

各モジュールの内部構造:

```text
modules/{name}/src/
├── core/     # no_std 実装（ヒープのみ、OS非依存）
└── std/      # std 依存の拡張（Tokio等）
```

## Scala → Rust 変換ルール

### 型の対応

| Scala | Rust | 備考 |
|-------|------|------|
| `trait` | `trait` | 継承階層は合成に変換 |
| `sealed trait` + `case class` | `enum` | バリアント網羅性をコンパイラが保証 |
| `class` | `struct` | |
| `object` | `impl` ブロック or モジュールレベル関数 | |
| `implicit` パラメータ | `TB: RuntimeToolbox` ジェネリクス | |
| `Option[T]` | `Option<T>` | |
| `Future[T]` | `impl Future<Output = T>` or `Pin<Box<dyn Future>>` | |
| `Try[T]` / `Either[L, R]` | `Result<T, E>` | |
| `akka.Done` | `()` | |
| `NotUsed` | `StreamNotUsed` | streams モジュール固有 |

### 設計パターンの対応

| Pekko | fraktor-rs | 変換ルール |
|-------|-----------|-----------|
| `ActorRef[T]` | `TypedActorRefGeneric<M, TB>` | メッセージ型 M が第1パラメータ |
| `Props` | `PropsGeneric<TB>` | TB パラメータ追加 |
| `Behavior[T]` | `Behavior<M, TB>` | `*Generic` サフィックスなし、`<M, TB>` 順 |
| `ActorSystem` | `ActorSystemGeneric<TB>` | TB パラメータ追加 |
| `ActorMaterializer` | `ActorMaterializerGeneric<TB>` | trait `Materializer` も別途存在 |
| `ActorContext[T]` | `TypedActorContextGeneric<'a, M, TB>` | ライフタイム + メッセージ型 + TB |
| メソッドチェーン | メソッドチェーン | 所有権移動に注意 |
| コンパニオンオブジェクト | `impl` ブロック | ファクトリメソッド |

**パラメータ順の原則**: メッセージ型 `M` が先、`TB` が後。`TB` にはデフォルト型パラメータ（`NoStdToolbox`）が設定されている場合がある。

### 命名規約

| Pekko | fraktor-rs | 例 |
|-------|-----------|-----|
| `camelCase` メソッド | `snake_case` メソッド | `mapAsync` → `map_async` |
| `PascalCase` 型 | `PascalCase` 型 | `Source` → `Source` |
| ジェネリクス付き | `*Generic` サフィックス | `ActorCell` → `ActorCellGeneric<TB>` |
| 型エイリアス（TB固定） | サフィックスなし | `type ActorCell = ActorCellGeneric<StdToolbox>` |

**例外**: `Behavior<M, TB>` のように `*Generic` サフィックスを持たない型もある。既存コードの命名に従うこと。

## fraktor-rs 固有の制約

### no_std / std 分離

- `core/` モジュール: `#![no_std]` + `extern crate alloc`
- `std/` モジュール: `std` 依存OK（Tokio, ネットワーク等）
- `core/` で `std::` を直接使用禁止（`cfg-std-forbid` lint で強制）

### RuntimeToolbox (TB) パターン

すべてのジェネリック型は `TB: RuntimeToolbox` パラメータを持つ。
これにより `no_std` と `std` で同一APIを提供する。

```rust
// ジェネリック版（core/）— デフォルト型パラメータ NoStdToolbox
pub struct XyzGeneric<TB: RuntimeToolbox = NoStdToolbox> { /* ... */ }

// 具象版（std/）— StdToolbox で固定
pub type Xyz = XyzGeneric<StdToolbox>;
```

| Toolbox | 用途 | 配置 |
|---------|------|------|
| `NoStdToolbox` | no_std 環境のデフォルト | `modules/utils/src/core/` |
| `StdToolbox` | std 環境（Tokio統合） | `modules/utils/src/std/` |

### AShared パターン（共有ラッパー）

`&mut self` メソッドを持つ型を複数箇所から共有する場合:

```rust
// ロジック本体
pub struct XyzGeneric<TB: RuntimeToolbox> { /* state */ }

// 共有ラッパー
#[derive(Clone)]
pub struct XyzSharedGeneric<TB: RuntimeToolbox> {
    inner: ArcShared<ToolboxMutex<XyzGeneric<TB>, TB>>,
}
```

### Dylint lint（8つ）

| lint | 強制内容 |
|------|---------|
| mod-file | モジュールは `mod.rs` ではなくディレクトリ構造 |
| module-wiring | `mod` 宣言の整合性 |
| type-per-file | 1ファイル1公開型 |
| tests-location | テストは `{type}/tests.rs` に配置 |
| use-placement | FQCN import |
| rustdoc | `///` doc コメント必須 |
| cfg-std-forbid | core/ での std 使用禁止 |
| ambiguous-suffix | 禁止サフィックス検出 |

### CQS 原則

- Query: `&self` + 戻り値
- Command: `&mut self` + `()` or `Result<(), E>`
- 違反が必要な場合は人間の許可を得ること

## 実装時の手順

1. Pekko参照実装（`references/pekko/`）の該当ソースを読む
2. 対応するfraktor-rsモジュール（`modules/{name}/`）の現状を確認
3. Scala → Rust変換ルールに従ってAPIを設計
4. `core/` に no_std 実装を配置
5. 必要に応じて `std/` に具象型エイリアスを追加
6. テストを `{type}/tests.rs` に作成
7. `./scripts/ci-check.sh dylint -m <module>` で lint チェック
