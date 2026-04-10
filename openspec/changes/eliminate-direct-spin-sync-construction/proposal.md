## Why

`DebugActorLockProvider` を使って same-thread 再入やロック順序の問題を検出したいが、actor-* の production code には `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` の直呼びがまだ残っている。そのため actor system の lock provider を debug 実装へ差し替えても検出対象が一部に限られ、デッドロック調査の網羅性が壊れている。

## What Changes

- actor-* の production code からの `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` の直接使用に加え、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` や `*::new_with_builtin_lock(...)` のような固定 backend 指定・固定 backend alias も原則禁止し、差し替え可能な生成境界へ寄せる
- actor-system 管理下の共有ロック生成は `ActorLockProvider` 経由に統一し、`BuiltinSpin` / `Std` / `Debug` の切替漏れをなくす
- actor-core の no_std runtime state で lock family 切替を要求する箇所は、module-local factory 単独ではなく provider から受け取る閉じた constructor boundary へ寄せる
- actor-* 内の provider 管理外 shared state は、provider から materialize された handle を受け取る shared wrapper または明示的な built-in 例外へ整理し、backend 直結をやめる
- **BREAKING** production code における「直 `SpinSync*::new`」および「固定 `SpinSync*` driver 指定」を許す暗黙契約を廃止し、backend 実装層または明示的な例外箇所に閉じ込める
- `SpinSync*` 直構築、固定 driver 指定、および固定 backend alias の残存を CI で検出できるよう、機械的な禁止ルールを追加する

## Capabilities

### New Capabilities
- `actor-lock-construction-governance`: actor-* の production code の lock 構築を差し替え可能な生成境界へ集約し、backend 直構築と固定 driver 指定を CI で禁止する

### Modified Capabilities
- `dispatcher-trait-provider-abstraction`: actor dispatcher / executor / mailbox / sender の共有ロック構築が `ActorLockProvider` を唯一の actor-system 境界として使う
- `actor-system-default-config`: default / custom actor system が選択した lock family を shared wrapper 群へ一貫して反映する
- `actor-runtime-safety`: deadlock 調査用 debug lock family への切替で runtime safety 検証が一部漏れにならないことを要求する

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/system/lock_provider/`
  - `modules/actor-core/src/core/kernel/dispatch/`
  - `modules/actor-core/src/core/kernel/actor/`
  - `modules/actor-core/src/core/typed/`
  - `modules/actor-adaptor-std/src/std/system/lock_provider/`
  - `lints/`
- 影響内容:
  - actor-system scoped な shared state は provider 経由へ統一
  - actor-* 内で直 `SpinSync*::new`、固定 `SpinSync*` driver 指定、または `new_with_builtin_lock` 系 alias を使っている production state は provider / provider から受け取る constructor boundary / shared wrapper へ移行する
  - CI が「差し替え漏れ」を機械的に落とすようになる
- 非目標:
  - tests 内の簡易な `SpinSync*::new` まで即時全面禁止すること
  - `dyn ActorLockProvider` を generic 化すること
  - actor-* 以外のモジュールに手を付けること
