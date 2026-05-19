## Context

現在の actor-* では、actor-system hot path の一部は `ActorLockProvider` で lock family を差し替えられるが、production code には `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` の直接呼び出し、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 backend 指定、さらに `ExecutorShared::new_with_builtin_lock(...)` / `ActorRefSenderShared::new_with_builtin_lock(...)` のような fixed-family helper alias が残っている。これにより `DebugActorLockProvider` を使っても same-thread 再入やロック順序の問題を全面的には観測できず、「debug lock family に切り替えたつもりでも一部は固定 backend のまま」という漏れが発生する。

一方で、この漏れを解消するために `dyn ActorLockProvider` をやめて generic 化すると、`ActorSystemConfig`、`SystemState`、dispatcher configurator、spawn 経路まで型引数が伝播し、公開面と設定面が過剰に汚染される。さらに `LockDriverFactory` / `RwLockDriverFactory` は compile-time の型選択 seam であり、`no_std` な actor-core が runtime override された debug/std family をそれ単独で受け取る手段にはならない。必要なのは generic provider ではなく、「actor-* の production の lock 構築が必ず差し替え境界を通る」ことと、「その漏れを CI が落とす」ことである。

## Goals / Non-Goals

**Goals:**
- production code からの直接 `SpinSync*::new` と固定 `SpinSync*` driver 指定を差し替え可能な構築境界へ集約する
- production code からの `*::new_with_builtin_lock(...)` など fixed-family helper alias を差し替え可能な構築境界へ集約する
- actor-system 管理下の lock family 切替は `ActorLockProvider` で継続し、`dyn` は維持する
- actor-core の no_std runtime state が debug/std family を必要とする場合は、provider から受け取る閉じた constructor boundary を通す
- actor-* 内の provider 管理外 shared state も、provider から受け取る handle / constructor boundary または明示的な built-in 例外へ整理し、debug family 切替漏れをなくす
- CI が production における backend 直構築を機械的に検出できるようにする

**Non-Goals:**
- `ActorLockProvider` を generic trait に作り替えること
- `dyn ActorLockProvider` を廃止すること
- tests 内の簡易な `SpinSync*::new` まで即時全面禁止すること
- `SpinSyncMutex` / `SpinSyncRwLock` 自体を削除すること
- actor-* 以外のモジュールに手を付けること

## Decisions

### 1. `ActorLockProvider` は actor-system scoped の object-safe 境界として維持する

actor runtime の dispatcher / executor / mailbox / sender は、引き続き `ActorLockProvider` で構築する。これは actor system ごとに異なる lock family を選べる既存境界であり、`dyn` をやめると型引数が `ActorSystemConfig` から configurator 群まで露出してしまうため採らない。

代わりに、各 concrete provider (`BuiltinSpinLockProvider` / `StdActorLockProvider` / `DebugActorLockProvider`) の実装内部に `create_lock` / `create_rw_lock` の helper を置き、object-safe な 4 メソッド実装の重複だけを局所化する。

代替案:
- `ActorLockProvider` を generic 化する: 型引数が広範囲に漏れるため不採用
- `type DataType` を使う: 複数の無関係な lock payload 型を扱えないため不採用

### 2. actor-core の no_std runtime state は provider から受け取る constructor boundary で family を選ぶ

actor-core は production で std adapter に依存しないため、`DebugSpinSyncMutex` / `StdSyncMutex` を module-local factory が直接選ぶ設計は成立しない。debug/std family へ切り替えたい runtime-owned state は、actor system bootstrap が保持している `ActorLockProvider` から materialize された shared handle、または provider に追加した bounded な concrete constructor API を通して受け取る。

具体的には、現在 `ArcShared<SpinSyncMutex<T>>` や `SpinSyncMutex<T>` を直接 field に持つ型は、必要に応じて:

- provider が返す concrete shared wrapper / shared handle
- provider が返した shared handle を保持する `*Shared` wrapper
- 明示的に built-in family へ閉じ込める module-private constructor

へ寄せる。

重要なのは「抽象名を増やすこと」ではなく、「production caller が backend concrete を直接 new しないこと」と、「runtime override が必要な backend choice を actor-core ローカルで決めないこと」である。単一型の private state であっても、debug family 差し替え対象にしたいなら provider から受け取る constructor 境界を持たせる。

代替案:
- compile-time の `LockDriverFactory` / `RwLockDriverFactory` だけで切り替える: runtime override を表現できず、actor-core から std/debug family を選べないため不採用
- workspace-wide な単一 `LockProvider` を導入する: actor-* 以外まで巻き込んで境界が過度に結合するため不採用
- すべて generic provider へ寄せる: actor-system scoped という責務を保てても型汚染が大きすぎるため不採用

### 3. fixed-family helper alias も backend concrete と同じ統治対象として扱う

`*::new_with_builtin_lock(...)` は call site から `SpinSync*` を隠しているだけで、lock family を固定してしまう点では `new_with_driver::<SpinSync*>` と同じである。そのため governance と lint の対象は direct `SpinSync*::new` と固定 driver 指定だけでなく、fixed-family helper alias も含める。

許可されるのは次だけとする。

- backend 実装層で concrete family を閉じ込める helper
- provider 実装が built-in family を materialize するための helper
- debug/std 切替対象外であることを文書化した built-in 例外

公開 wrapper の convenience constructor を残す場合でも、actor-* production caller から自由に呼べる状態は許容しない。

### 4. production の direct `SpinSync*::new`、固定 driver 指定、fixed-family helper alias は lint で禁止する

手作業レビューでは漏れを防げないため、CI に禁止ルールを追加する。対象は production Rust file での `SpinSyncMutex::new(...)` / `SpinSyncRwLock::new(...)` の直接使用、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 backend 指定、および `*::new_with_builtin_lock(...)` のような fixed-family helper alias であり、backend 実装ファイル、provider 実装、factory 実装、必要な低レベル例外だけを allow-list で許可する。

この lint は「どの抽象を使うか」を強制するのではなく、「backend concrete の直 new、固定 driver 指定、fixed-family alias は不可」という最小規則だけを課す。そうすることで actor-* 内で constructor boundary の形状は局所最適化しつつ、差し替え漏れだけを確実に防げる。

代替案:
- 口頭規約に留める: 漏れ検出ができないため不採用
- 既存 clippy 設定だけで対処する: 呼び出し位置や allow-list 制御が不足するため不採用

### 5. 置換対象は「debug family で観測したい production state」から優先する

全面一括変換ではなく、`DebugActorLockProvider` で検出したい runtime state から順に寄せる。優先順位は次のとおり。

1. actor-core の runtime shared state / mailbox / subscriber path
2. actor-core typed / dispatch / event 系の helper state
3. actor-adaptor-std の debug 観測 path

この順にすることで、まずデッドロック調査の主対象である runtime path の漏れを潰せる。

## Risks / Trade-offs

- [Risk] provider-sensitive な runtime-owned state ごとに constructor API が増える → Mitigation: `ActorLockProvider` へ generic API は入れず、bootstrap で必要な concrete surface だけを閉じた集合で追加する
- [Risk] module ごとに constructor boundary の形状が異なり、抽象が増える → Mitigation: 禁止するのは backend 直 new / alias だけに留め、共通抽象は本当に必要な場所にだけ導入する
- [Risk] allow-list が広すぎると lint が骨抜きになる → Mitigation: backend 実装層と明示的 factory 実装だけに限定し、通常の caller は必ず失敗させる
- [Risk] actor-* 内の private state を一度に直しすぎて scope が膨らむ → Mitigation: debug family で観測したい runtime state を優先し、その他は後続タスクへ分割する
- [Risk] 既存 `SharedLock::new_with_driver::<SpinSyncMutex<_>>` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>` と `new_with_builtin_lock` alias が散在し、provider 経由と固定 family 指定が混在する → Mitigation: actor-system 管理下では provider 経由を優先し、固定 family 指定の残件を棚卸し対象として明示する

## Migration Plan

1. production の direct `SpinSync*::new`、固定 `SpinSync*` driver 指定、fixed-family helper alias の使用箇所を棚卸しし、allow-list 候補と置換対象を分類する
2. actor-core で provider 経由へ寄せられる箇所を `ActorLockProvider` 利用に統一し、runtime-owned state ごとに「provider が返す concrete surface」を決める
3. actor-* 内の provider 管理外 state を「provider-sensitive な runtime-owned state」と「明示的な built-in 例外」に分け、前者は provider から受け取る constructor boundary へ、後者は allow-list へ整理する
4. direct `SpinSync*::new`、固定 `SpinSync*` driver 指定、fixed-family helper alias を禁止する lint を追加し、allow-list を最小化する
5. `DebugActorLockProvider` を使う system test と、lint が漏れを落とす test を追加する

## Open Questions

- `SpinSync*` 直構築、固定 driver 指定、fixed-family alias の例外をどこまで許可するかは、最初の棚卸し結果を見て allow-list を確定する
