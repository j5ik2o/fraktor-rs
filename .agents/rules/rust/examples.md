# Rust Rules - Examples

## Principles Examples

### 内部可変性禁止
**Good:**
```rust
// modules/actor-core/src/core/kernel/actor/actor_shared.rs（一部）
// ロジック本体は &mut self で設計、共有が必要な箇所だけ Shared ラッパーを介在させる
#[derive(Clone)]
pub struct ActorShared {
  inner: SharedLock<Box<dyn Actor + Send>>,
}

impl ActorShared {
  pub fn new(actor: Box<dyn Actor + Send>) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(actor))
  }
}
```
**Bad:**
```rust
// 内部可変性で &self に偽装。借用チェッカの保護が無効化され、
// CQS 判定もしづらくなる。`clippy.toml` で std::sync::Mutex の直接使用も disallowed。
pub struct ActorShared {
  inner: std::sync::Mutex<Box<dyn Actor + Send>>,
}

impl ActorShared {
  pub fn step(&self) { /* &self のまま中で lock して書き換え */ }
}
```

### CQS 厳守
**Good:**
```rust
// 読み取りは &self、更新は &mut self、戻り値は分離する
fn process(&mut self) {
  self.state += 1;
}
fn processed_data(&self) -> ProcessedData {
  ProcessedData::new(self.state)
}
```
**Bad:**
```rust
// 状態変更しつつ値を返す CQS 違反。Vec::pop 相当の不可避ケース以外は許容しない。
fn process_and_get(&mut self) -> ProcessedData {
  self.state += 1;
  ProcessedData::new(self.state)
}
```

### mod.rs 禁止
**Good:**
```text
modules/actor-core/src/core/kernel/
├── actor.rs           # wiring（mod 宣言と pub use のみ）
├── actor/
│   ├── actor_cell.rs
│   ├── actor_shared.rs
│   └── pid.rs
```
```rust
// modules/actor-core/src/core/kernel/actor.rs
mod actor_cell;
mod actor_shared;
mod pid;

pub use actor_cell::ActorCell;
pub use actor_shared::ActorShared;
pub use pid::Pid;
```
**Bad:**
```text
modules/actor-core/src/core/kernel/actor/
└── mod.rs   # mod-file-lint でエラー
```

### テストは sibling `_test.rs` 分離
**Good:**
```rust
// modules/actor-core/src/core/kernel/actor/actor_cell.rs
#[cfg(test)]
#[path = "actor_cell_test.rs"]
mod tests;

use ...
```
ファイル隣の `actor_cell_test.rs` に `use super::ActorCell;` で参照する。
**Bad:**
```rust
// インライン定義は tests-location-lint でエラー
#[cfg(test)]
mod tests {
  use super::*;
  #[test] fn it_works() { /* ... */ }
}
```

### コード本体での FQCN 禁止
**Good:**
```rust
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, WeakShared};

let shared = ArcShared::new(value);
```
**Bad:**
```rust
// redundant-fqcn-lint でエラー（`use` 宣言内の FQCN は許可）
let shared = fraktor_utils_core_rs::core::sync::ArcShared::new(value);
```

### 曖昧サフィックス禁止
**Good:**
```rust
pub struct ActorPathRegistry { /* ... */ }      // データ保持: *Registry
pub struct CoordinatedShutdownInstaller { /* ... */ }  // 仲介: *Installer
pub struct AffinityExecutor { /* ... */ }       // 実行: *Executor
```
**Bad:**
```rust
pub struct ActorManager { /* ... */ }    // ambiguous-suffix-lint でエラー
pub struct StringUtil { /* ... */ }      // 同上
pub struct PathService { /* ... */ }     // 同上
```

### ドキュメント言語の使い分け
**Good:**
```rust
//! Untyped actor runtime kernel packages.

/// Identifies an actor instance within the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pid {
  // 後方互換性は不要なので、必要なら value のレイアウトを変えてよい
  value: u64,
  generation: u32,
}
```
**Bad:**
```rust
//! アクターランタイムのカーネルパッケージ。  // rustdoc-lint でエラー（rustdoc は英語）

/// アクターのプロセス識別子。            // 同上
pub struct Pid { /* ... */ }
```

### 戻り値の握りつぶし禁止
**Good:**
```rust
match mailbox.try_send(msg) {
  Ok(()) => {}
  Err(err) => {
    tracing::warn!(error = %err, "mailbox send failed; dropping message");
    metrics.record_mailbox_drop();
  }
}
```
**Bad:**
```rust
let _ = mailbox.try_send(msg);  // let-underscore-must-use / let-underscore-forbid-lint で検出
mailbox.try_send(msg).ok();      // エラー情報を握りつぶす
```

### `*-core` クレートの no_std 強制
**Good:**
```rust
// modules/actor-core/src/lib.rs
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

extern crate alloc;
```
**Bad:**
```rust
// no_std 指定がない、または std を直接 use する core クレート
use std::collections::HashMap;  // cfg-std-forbid-lint で検出
```

## Project-specific Examples

### `fraktor_utils_core_rs::core::sync::ArcShared<T>`
```rust
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, WeakShared};

// `Arc::new` ではなく `ArcShared::new` を使う。`feature = "force-portable-arc"` で
// portable_atomic_util::Arc にも切り替わるため、対象アーキテクチャ依存を吸収できる。
let shared: ArcShared<dyn ActorBackend> = ArcShared::new_dyn(backend);
```

### `SharedLock<T>` / `SharedRwLock<T>` + `SharedAccess`
```rust
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

// 書き込み主体: SharedLock。読み込み主体なら SharedRwLock + DefaultRwLock<_>
let inner = SharedLock::new_with_driver::<DefaultMutex<_>>(state);

// ガードを外部へ返さず、クロージャ内に閉じる
inner.with_write(|state| state.advance());
let snapshot = inner.with_read(|state| state.snapshot());
```

### `*Shared` / `*Handle` 命名
```rust
// 薄い同期ラッパー（lifecycle 責務なし）
pub struct ActorShared { inner: SharedLock<Box<dyn Actor + Send>> }

// ライフサイクル / 管理責務（起動・停止・複数構成要素の束ね）
pub struct ActorPathHandle { /* ... */ }
```
