# actor-lock-construction-governance Specification

## Purpose
TBD - created by archiving change eliminate-direct-spin-sync-construction. Update Purpose after archive.
## Requirements
### Requirement: actor-* の production lock construction は差し替え可能な境界を通らなければならない

actor-* の production code の lock 構築は、選択済み lock family を materialize する provider、provider から受け取る constructor boundary、factory、または shared wrapper constructor を通らなければならない（MUST）。`SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` のような backend concrete の直接構築、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 backend 指定、および `*::new_with_builtin_lock(...)` のような fixed-family helper alias を production caller が行ってはならない（MUST NOT）。

#### Scenario: actor-* の production caller は backend concrete または fixed-family alias を直接使わない
- **WHEN** actor-* の production Rust file が shared state を構築する
- **THEN** その file は `SpinSyncMutex::new(...)` または `SpinSyncRwLock::new(...)` を直接呼ばない
- **AND** その file は `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` または `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 backend 指定を行わない
- **AND** その file は `*::new_with_builtin_lock(...)` のような fixed-family helper alias を直接呼ばない
- **AND** lock 構築は provider、provider から受け取る constructor boundary、factory、または shared wrapper constructor に閉じる

#### Scenario: actor-core の no_std runtime-owned state は provider が選んだ family を使う
- **WHEN** actor-core の production path が debug/std family 切替対象の runtime-owned shared state を構築する
- **THEN** backend choice は actor-core の module-local constructor で決まらない
- **AND** その state は provider が materialize した concrete surface または provider から受け取る constructor boundary を通して構築される
- **AND** actor-core が std/debug backend concrete を直接参照しない

#### Scenario: debug lock family への切替で差し替え漏れが残らない
- **WHEN** runtime が debug lock family を選択して起動する
- **THEN** その runtime path の production lock 構築は選択済み family を使う
- **AND** hard-coded `SpinSync*` backend または固定 `SpinSync*` driver 指定による silent bypass が存在しない

### Requirement: backend direct construction の残存は CI が検出しなければならない

workspace は、actor-* の production code に残った backend direct construction、固定 driver 指定、fixed-family helper alias を CI で検出しなければならない（MUST）。許可されるのは backend 実装層、provider 実装、明示的な factory 実装、または文書化された例外箇所だけである（MUST）。

#### Scenario: actor-* production file の fixed-family lock construction は CI で失敗する
- **WHEN** actor-* の production Rust file が allow-list 外で `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` を使う、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 backend 指定を行う、または `*::new_with_builtin_lock(...)` のような fixed-family helper alias を呼ぶ
- **THEN** lint または同等の CI ルールは build を失敗させる
- **AND** failure message は provider / provider から受け取る constructor boundary / factory / shared wrapper 経由へ寄せるべきことを示す

#### Scenario: backend 実装層は例外として許可される
- **WHEN** `SpinSyncMutex` / `SpinSyncRwLock` 自身の実装ファイル、または明示的に管理された factory 実装が backend concrete を構築する
- **THEN** CI はその箇所を違反として扱わない
- **AND** 通常 caller と区別できる allow-list が定義されている

### Requirement: actor-* の production code は primitive lock crate を直接 use してはならない

`actor-*` の production code は、`critical-section`、`spin`、`parking_lot` などの primitive lock crate、および `std::sync::Mutex` / `std::sync::RwLock` を、同期プリミティブを構築するために直接 import / 構築してはならない（MUST NOT）。同期プリミティブを必要とする shared state は、`utils-core` が提供する `SharedLock` / `SharedRwLock` 抽象と `DefaultMutex` / `DefaultRwLock` driver を通して構築されなければならない（MUST）。

本 requirement は `compile-time-lock-backend` spec の `DefaultMutex` 利用要件、および本 spec 既存の「fixed-family lock construction 禁止」要件を補完する。本要件は primitive lock crate の利用境界（誰がどの primitive crate を直接 use してよいか）を明示する役割を持つ。

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

### Requirement: actor-* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない

`actor-*` クレートの `Cargo.toml` は、`critical-section`、`spin`、`parking_lot` などの primitive lock crate を `[dependencies]` に直接依存として宣言してはならない（MUST NOT）。これらの crate への依存は、`utils-core` を通した推移的依存として表現されなければならない（MUST）。

ただし以下は例外として許可する:

- `portable-atomic` のような low-level utility crate が引き込む推移的依存
- 各クレートの `[dev-dependencies]` に test/bench 用の impl provider 取得目的で記述される `critical-section = { features = ["std"] }` 等のエントリ（production 依存ではないため `[dependencies]` 直接宣言禁止には該当しない）
- `showcases/std` のような実行バイナリ crate の `[dependencies]` における impl provider 取得目的の記述（バイナリ側が impl 選択責任を持つ標準作法に基づく）

impl provider 取得は各バイナリが `[dev-dependencies]` または `[dependencies]` で直接宣言する形に統一する。

#### Scenario: actor-core の Cargo.toml は critical-section を `[dependencies]` 直接依存として持たない

- **WHEN** `modules/actor-core/Cargo.toml` の `[dependencies]` セクションで `critical-section` エントリを検査する
- **THEN** `critical-section` エントリは存在しない
- **AND** `critical-section` への依存は `portable-atomic = { features = ["critical-section"] }` のような推移的経路でのみ表現される
- **AND** `[dev-dependencies]` には `critical-section = { workspace = true, features = ["std"] }` が impl provider 取得目的で記述されてよい（actor-core 自身の `cargo test` で必要）

#### Scenario: actor-core の Cargo.toml は spin を `[dependencies]` 直接依存として持たない

- **WHEN** `modules/actor-core/Cargo.toml` の `[dependencies]` セクションで `spin` エントリを検査する
- **THEN** `spin` エントリは存在しない
- **AND** `spin` への依存は `fraktor-utils-core-rs` 経由の推移的経路でのみ表現される
- **AND** `actor-core` の production code 内の write-once + lock-free read 用途（旧 `spin::Once<T>` 利用箇所）は `fraktor-utils-core-rs` が提供する `SyncOnce<T>` 抽象を通して構築される

#### Scenario: actor-* の他クレートも同じ規約に従う

- **WHEN** `fraktor-actor-adaptor-std-rs`、`fraktor-cluster-*-rs`、`fraktor-remote-*-rs`、`fraktor-stream-*-rs`、`fraktor-persistence-*-rs` の `Cargo.toml` を読む
- **THEN** いずれも `critical-section`、`spin`、`parking_lot` を `[dependencies]` 直接宣言として持たない
- **AND** これらのクレートが同期プリミティブを必要とする場合は `fraktor-utils-core-rs` 経由で取得する
- **AND** test/bench で `critical-section` の `std` impl が必要な場合は `[dev-dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する

#### Scenario: 各バイナリは impl provider を直接宣言する

- **WHEN** `actor-*` 配下のテスト（`[[test]]`）、bench、または `showcases/std` 等の実行バイナリ crate が `critical-section` の impl を必要とする
- **THEN** 当該 crate は `[dev-dependencies]` または `[dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する
- **AND** `actor-*` の library crate の feature flag（例: `test-support`）を経由した自動配給には依存しない

