## 1. 棚卸しとガードレール

- [ ] 1.1 actor-* の production code に残っている `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` の直使用、固定 `SpinSync*` driver 指定、`new_with_builtin_lock` 系 alias の箇所を棚卸しし、allow-list 候補と置換対象に分類する
- [ ] 1.2 backend 実装層、provider 実装、明示的 factory 実装だけを許可する「direct `SpinSync*::new` / 固定 driver 指定 / fixed-family alias の禁止」lint の仕様を確定する
- [ ] 1.3 lint 追加前に、変更対象の module ごとに「provider が返す concrete surface へ寄せるか」「provider から受け取る constructor boundary に寄せるか」「明示的 built-in 例外へ残すか」を決めて migration メモを作る

## 2. Actor Runtime の provider 境界統一

- [ ] 2.1 `BuiltinSpinLockProvider` / `StdActorLockProvider` / `DebugActorLockProvider` に module-local な `create_lock` / `create_rw_lock` helper を整理し、既存の object-safe API と runtime-owned concrete surface 構築をその helper 経由へ統一する
- [ ] 2.2 actor-system 管理下で固定 `SpinSync*` driver 指定または `new_with_builtin_lock` alias を使っている production wiring を `ActorLockProvider` 経由へ置換する
- [ ] 2.3 `ActorSystemConfig::with_lock_provider(...)` の override が default dispatcher seed、spawn path、mailbox shared set、および provider-sensitive な bootstrap surface まで一貫して反映されることを test で確認する

## 3. Actor Runtime 内の provider 管理外 state の整理

- [ ] 3.1 actor-core の event / typed / helper state を「provider-sensitive な runtime-owned state」と「明示的 built-in 例外」に分類し、前者は provider から受け取る constructor boundary または provider materialized handle 経由へ置換する
- [ ] 3.2 actor-core 内の private slot と helper path を見直し、`new_with_builtin_lock` を含む fixed-family helper caller は constructor boundary へ移し、純粋な低レベル例外だけを allow-list へ残す
- [ ] 3.3 actor-adaptor-std の debug 観測 path が actor-core 側の置換後 runtime state を確実に通ることを test で確認する

## 4. CI 強制と検証

- [ ] 4.1 production code の direct `SpinSync*::new`、固定 `SpinSync*` driver 指定、fixed-family helper alias を禁止する lint を追加し、allow-list 外の使用を CI failure にする
- [ ] 4.2 `DebugActorLockProvider` または同等の debug lock family で、same-thread 再入検知の適用範囲に漏れがないことを確認する test を追加する
- [ ] 4.3 lint と runtime test の両方が通ることを確認し、`./scripts/ci-check.sh ai all` で最終検証する
