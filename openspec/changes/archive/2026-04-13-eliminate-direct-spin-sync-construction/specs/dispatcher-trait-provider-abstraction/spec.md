## ADDED Requirements

### Requirement: actor runtime の shared wrapper 構築は provider 境界に集約されなければならない

actor runtime が使う dispatcher、executor、actor-ref sender、mailbox lock bundle の shared wrapper 構築は `ActorLockProvider` 境界に集約されなければならない（MUST）。actor-system 管理下の production wiring が `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 concrete driver 指定や、`*::new_with_builtin_lock(...)` のような fixed-family helper alias を直接行ってはならない（MUST NOT）。

#### Scenario: actor system scoped shared wrapper は provider 経由で materialize される
- **WHEN** actor system が dispatcher、executor、actor-ref sender、mailbox shared set を構築する
- **THEN** それらは `ActorLockProvider` から materialize される
- **AND** caller は concrete lock family 名を直接指定しない
- **AND** caller は fixed-family helper alias で built-in backend を迂回指定しない

#### Scenario: debug provider 選択時に actor runtime 全体で同じ family を使う
- **WHEN** actor system が debug 用 `ActorLockProvider` を設定して起動する
- **THEN** dispatcher、executor、actor-ref sender、mailbox shared set はその provider family で構築される
- **AND** actor runtime の一部だけが builtin spin backend に固定されない

#### Scenario: provider-sensitive な bootstrap surface は provider が選んだ family を受け取る
- **WHEN** actor-core の bootstrap path が dispatcher / mailbox 以外の runtime-owned shared surface を構築する
- **THEN** その path は `ActorLockProvider` が返す concrete surface または provider から受け取る constructor boundary を使う
- **AND** actor-core の caller は `new_with_builtin_lock(...)` や `new_with_driver::<SpinSync*>` で family を固定しない
