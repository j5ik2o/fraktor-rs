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

