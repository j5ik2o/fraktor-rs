## Why

Rust ではエラー型を `*Error` 接尾辞で命名するのが標準慣習であり、`std::io::Error`、`serde::de::Error`、`Box<dyn std::error::Error>` などエコシステム全体がこの形に揃っている。一方、fraktor-rs には Pekko / Java 由来の名前をそのまま移植した結果、`*Exception` 接尾辞の公開エラー型が 2 件残存している:

- `actor-core` の `ThrowableNotSerializableException` (`modules/actor-core/src/core/kernel/serialization/throwable_not_serializable_exception.rs`)
- `persistence-core` の `DurableStateException` (`modules/persistence-core/src/core/durable_state_exception.rs`)

両者とも周辺の Rust 側エラー型 (`NotSerializableError`、`SerializationError`、`SerializerIdError`、`SerializationBuilderError` など) は既に `*Error` 命名で揃っており、この 2 件だけが浮いている。さらに `docs/gap-analysis/remote-gap-analysis.md` の Phase 1 (trivial / easy) でも `ThrowableNotSerializableException` 相当として gap が立っているが、実装は既に存在しており残るのは命名整合のみ。

本 change は Rust 慣習および周辺型と整合させるため、公開エラー型から `*Exception` 接尾辞を一掃する。

## What Changes

### 型・ファイル・モジュールの rename

| Before | After |
|--------|-------|
| `ThrowableNotSerializableException` (struct) | `ThrowableNotSerializableError` |
| `throwable_not_serializable_exception.rs` | `throwable_not_serializable_error.rs` |
| `DurableStateException` (enum) | `DurableStateError` |
| `durable_state_exception.rs` | `durable_state_error.rs` |
| `durable_state_exception/` (tests dir) | `durable_state_error/` |

`Throwable` 接頭辞は Pekko 元概念 (Java `Throwable` を serialize できなかった代替 payload) を識別する語として保持する。Rust に `Throwable` 概念は存在しないが、Pekko 互換層 serialization 側で「元 throwable の class name + message を保持する代替」という役割を端的に表す語として有用。

### 参照箇所の追従更新

- `modules/actor-core/src/core/kernel/serialization.rs` の `mod` / `pub use` 宣言
- `modules/actor-core/src/core/kernel/serialization/error/tests.rs` の import / コンストラクタ呼び出し (3 箇所)
- `modules/persistence-core/src/core.rs` の `mod` / `pub use` 宣言
- `modules/persistence-core/src/core/durable_state_store.rs` の `Result<T, DurableStateException>` シグネチャ (2 箇所)
- `modules/persistence-core/src/core/durable_state_store_registry.rs` の import / 戻り値型 / ファクトリ呼び出し (5 箇所)
- `modules/persistence-core/src/core/durable_state_store_registry/tests.rs` (4 箇所)
- `modules/persistence-core/src/core/durable_state_error/tests.rs` (移動後・7 箇所、import + assert)

### gap analysis 表記の同期

- `docs/gap-analysis/remote-gap-analysis.md` Phase 1 表の `ThrowableNotSerializableException` 相当行を「対応済み (新名 `ThrowableNotSerializableError`)」に更新
- 表中の他箇所 (line 111) の `ThrowableNotSerializableException` 言及も新名に同期

### BREAKING

両型とも `pub` 公開のため workspace 外利用者がいる場合は破壊的変更になる。fraktor-rs は正式リリース前 (CLAUDE.md「リリース状況」参照) であり、後方互換は不要との方針が定められているため新名のみを残す。

### Non-Goals

- `actor-core/serialization` 全体の error 型再編 (例: `NotSerializableError` と `ThrowableNotSerializableError` の責務再整理) は対象外。本 change は単独の rename と命名整合に閉じる。
- `DurableStateException` の variant 名 (`GetObjectFailed`、`UpsertObjectFailed`、`DeleteObjectFailed`、`ChangesFailed`、`ProviderAlreadyRegistered`、`ProviderNotFound`) の見直しは対象外。`Failed` 接尾辞は variant としては許容範囲とし、本 change では型名のみ整える。
- `*Exception` 接尾辞の **テスト関数名** (`throwable_not_serializable_exception_preserves_*` 等) は本 change の対象外とするか、`*_error_*` に揃えて改名するかを design.md で決定する。
