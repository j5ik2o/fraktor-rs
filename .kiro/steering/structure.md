# プロジェクト構造

## 構成方針

workspace は runtime domain と portability layer で分ける。domain crate はふるまいを所有し、adaptor crate は host integration を所有する。新しいファイルや crate は、core contract と runtime 固有実装の境界を曖昧にせず、依存方向をより明確にするために追加する。

## ディレクトリパターン

### ドメイン別 module crate

**場所**: `modules/{domain}-{layer}`  
**目的**: runtime domain を portable core / typed facade / host adaptor の層に分ける。代表的な domain は utils、actor、persistence、remote、cluster、stream。  
**例**: `actor-core-kernel` は untyped actor kernel contract を定義し、`actor-adaptor-std` は Tokio/std binding を提供する。

### コアとアダプタの境界

**場所**: `modules/*-core-*` と `modules/*-adaptor-*`  
**目的**: core crate は trait、state machine、identifier、configuration、domain error を定義する。adaptor crate は executor、network、clock、lock、file、OS/runtime binding を提供する。  
**例**: remote core は address / association / wire contract を所有し、remote std adaptor は Tokio TCP transport と I/O worker を所有する。

### 実行可能 showcase

**場所**: `showcases/std`  
**目的**: runnable example と usage surface は module crate 配下ではなくここに置く。API flow を示す場合は showcase を優先する。  
**例**: actor、typed、stream、remote、persistence の scenario は executable std example として整理する。

### クレート横断テスト

**場所**: `tests/e2e` と `modules/*/tests`  
**目的**: crate 境界をまたぐ end-to-end behavior は e2e または crate-level integration test に置く。unit test は対象 module の近くに置く。  
**例**: actor runtime boot や adaptor boot flow で public runtime assembly を検証する。

### 仕様と計画

**場所**: `openspec`, `docs/plan`, `docs/gap-analysis`  
**目的**: runtime contract に影響するふるまい変更は spec または planning doc から始める。gap analysis は intended surface と reference implementation の差分を確認するために使う。  
**例**: cluster work では gap analysis と roadmap docs を使い、現在の Grain runtime priority と deferred parity work を分ける。

## 命名規約

- **crate**: `fraktor-{domain}-{layer}-rs`。workspace module directory と対応させる。
- **Rust file**: `snake_case.rs`。`mod.rs` は避ける。
- **公開型**: 明示的な project exception がない限り、1公開型につき1ファイル。
- **shared wrapper**: 薄い共有所有 wrapper は `*Shared`、command/lifecycle handle は `*Handle` を使う。
- **Cargo feature**: `kebab-case`。feature 名は runtime capability または integration surface を表す。
- **rustdoc**: 英語。Markdown と通常コメントは、周辺ファイルが別方針でない限り日本語。

## インポートの整理

```rust
use fraktor_utils_core_rs::sync::ArcShared;

use crate::actor::ActorRef;
```

- `use` 宣言はファイル先頭に置く。
- code body では fully qualified path より imported name を優先する。
- public module wiring は薄く保つ。親 module は子 module 宣言と module boundary の公開に留め、子の公開型を無関係な親ファイルへ集約しない。

## コード構成原則

- `*-core` crate は `std` に依存しない。host concern は adaptor crate に移す。
- 新しい public domain primitive、error、shared wrapper、handle は原則として専用ファイルに置き、テスト対象なら sibling `*_test.rs` を置く。
- project lint が分離を要求する場合、source file の test は inline test module ではなく sibling `*_test.rs` に置く。
- reference implementation は semantics の確認に使うが、steering では source-copy structure より project-specific boundary を優先する。
- `.kiro/specs` は feature-specific decision を記録し、steering は feature をまたいで安定する project-wide pattern を記録する。

---
_ファイルツリーではなくパターンを記録する。ここに従う新規ファイル追加では steering 更新が不要になる粒度を保つ。_
