## MODIFIED Requirements

### Requirement: actor-* の production code は primitive lock crate を直接 use してはならない

`actor-*` の production code は、`critical-section`、`spin`、`parking_lot` などの primitive lock crate、および `std::sync::Mutex` / `std::sync::RwLock` を、同期プリミティブを構築するために直接 import / 構築してはならない（MUST NOT）。同期プリミティブを必要とする shared state は、`utils-core` が提供する `SharedLock` / `SharedRwLock` 抽象と `DefaultMutex` / `DefaultRwLock` driver を通して構築されなければならない（MUST）。

本 requirement は `compile-time-lock-backend` spec の `DefaultMutex` 利用要件、および本 spec 既存の「fixed-family lock construction 禁止」要件を補完する。本要件は primitive lock crate の利用境界（誰がどの primitive crate を直接 use してよいか）を明示する役割を持つ。

加えて、`actor-*` の production code は **ISR セーフな経路に見せかけながら内部で通常の lock を取る public API**（例: 名前に `_from_isr` / `_irq` / `isr_` 等を含むが実装が通常の `SharedLock::with_lock` 等を使うもの）を公開してはならない（MUST NOT）。真に ISR セーフな経路が必要な場合は、ISR セーフな backend（`DefaultMutex` の該当 feature variant 等）を伴う形で設計し、API 名と実装セマンティクスを一致させなければならない（MUST）。実装が追随していない段階で ISR セーフに見える名前の API を先行公開することは禁止する。

primitive lock crate の直接 use が許可されるのは以下のみとする:

- `utils-core` 内の backend 実装ファイル（`spin_sync_mutex.rs`、`std_sync_mutex.rs`、`checked_spin_sync_mutex.rs` など）
- `utils-core` 内の driver / factory / shared wrapper 実装
- テストコード（`#[cfg(test)]` module、`tests/` ディレクトリ、`benches/`）

#### Scenario: actor-* の production file は primitive lock crate を直接 use しない

- **WHEN** `actor-*` の production Rust file（`#[cfg(test)]` module、`tests/`、`benches/` 配下を除く）が同期プリミティブを必要とする shared state を構築する
- **THEN** その file は `use critical_section::{...};`、`use spin::{...};`、`use parking_lot::{...};`、`use std::sync::Mutex;`、`use std::sync::RwLock;` のような primitive lock crate / module からの import を行わない
- **AND** その file は `critical_section::Mutex::new(...)`、`critical_section::with(...)`、`spin::Mutex::new(...)`、`parking_lot::Mutex::new(...)`、`std::sync::Mutex::new(...)`、`std::sync::RwLock::new(...)` などの直接構築を行わない
- **AND** その shared state は `SharedLock::new_with_driver::<DefaultMutex<_>>(...)` または `SharedRwLock::new_with_driver::<DefaultRwLock<_>>(...)` を通して構築される

#### Scenario: backend 実装層と utils-core は例外として許可される

- **WHEN** `utils-core` 内の backend 実装ファイル（例: `spin_sync_mutex.rs`、`std_sync_mutex.rs`、`checked_spin_sync_mutex.rs`）または driver / factory / shared wrapper 実装が primitive lock crate / `std::sync` を直接 use する
- **THEN** その箇所は本 requirement の違反として扱わない
- **AND** allow-list は `utils-core/src/core/sync/` 配下に閉じる

#### Scenario: actor-* は ISR セーフに見せかけた通常ロック API を公開しない

- **WHEN** `actor-*` の production Rust file（`#[cfg(test)]` module、`tests/`、`benches/` 配下を除く）が `pub fn` を宣言する
- **THEN** その関数名が `_from_isr` / `_irq` / `isr_` 等の ISR セーフティを示唆する suffix / prefix を含むにもかかわらず、実装が `SharedLock::with_lock` / `SharedRwLock::with_read` / `with_write` など通常の lock acquisition を経由する、という組み合わせは存在しない
- **AND** 真に ISR セーフな経路が必要な場合は、対応する backend（`DefaultMutex` の ISR セーフ feature variant 等）の実装と整合した形で実装される
- **AND** `TickFeed::enqueue_from_isr` のように API 名だけ先行して中身が通常の `enqueue` と同一である public API は production に存在しない
