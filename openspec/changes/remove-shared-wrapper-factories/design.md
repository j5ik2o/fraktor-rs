## Context

現在の actor runtime は `ActorSharedFactory` 廃止後に、`MessageDispatcherSharedFactory`、`ExecutorSharedFactory`、`ActorRefSenderSharedFactory`、`MailboxSharedSetFactory` など多数の型別 `*SharedFactory` へ分解された。しかし concrete 実装の大半は `SharedLock::new_with_driver::<...>(...)` / `SharedRwLock::new_with_driver::<...>(...)` を hidden helper に包んでいるだけで、実装量と wiring の複雑さに対する実益が薄い。

今回の前提は明確で、`*SharedFactory` も `ActorLockFactory` trait も残さない。actor runtime の shared wrapper / shared state は builtin spin backend を直接指定して構築し、pre-release フェーズにふさわしく最小構成へ戻す。

## Goals / Non-Goals

**Goals:**
- actor runtime の型別 `*SharedFactory` を削除し、shared wrapper / shared state を direct builtin spin construction へ戻す
- `ActorSystemConfig` / `ActorSystemSetup` から shared runtime override seam を削除する
- `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` と `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を actor runtime の標準構築方法として再定義する
- std adapter 公開面から `shared_factory` module と std/debug factory concrete 型を削除する

**Non-Goals:**
- debug/std lock family 切替を別の abstraction で残すこと
- actor runtime の shared wrapper セマンティクスや `AShared` パターンを変更すること
- actor runtime 外の module に同じ方針を強制すること
- `CircuitBreakerSharedFactory<C>` のような type-indexed registry を今回まとめて削除すること

## Decisions

### 1. actor runtime の shared wrapper / shared state は builtin spin driver を直接指定して構築する

`MessageDispatcherShared`、`ExecutorShared`、`ActorRefSenderShared`、`ActorShared`、`ActorCellStateShared`、`ReceiveTimeoutStateShared`、`MessageInvokerShared`、`MailboxSharedSet`、priority queue state shared、`EventStreamShared`、`TickDriverControlShared` などの runtime-managed shared wrapper / shared state は、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` または `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を直接使って構築する。

型の既存 API に `from_shared_lock(...)` がある場合はそれを活かしてよいが、`new_with_lock_factory(...)` のような新たな seam は導入しない。重複が出る場合も、共有 factory trait ではなく file-local helper までに留める。

代替案:
- `ActorLockFactory` を残す: `*SharedFactory` の爆発を別名に縮退するだけで、今回の最終ゴールとずれるため不採用
- すべての型に統一 constructor abstraction を追加する: 追加 abstraction が YAGNI のため不採用

### 2. `SharedRwLock` を使う箇所も同じく direct builtin spin construction に戻す

`EventStreamShared`、`MessageInvokerShared`、`DeadLetterShared`、`SchedulerShared`、`SystemStateShared`、`serialization_registry` など `SharedRwLock` を使う箇所も、`SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を直接使う。`SharedLock` だけ factory を廃止し、`SharedRwLock` だけ別 seam を残すことはしない。

これにより「`*SharedFactory` を廃止したのに `SharedRwLock` だけ別戦略」という中途半端な状態を避ける。

代替案:
- `SharedRwLock` だけ別 seam を残す: 戦略が二重化し、理解コストだけ増えるため不採用
- `SharedRwLock` 利用箇所は今回対象外にする: 直接構築へ戻す方針が半端になるため不採用

### 3. actor system config から shared runtime override seam を削除する

`ActorSystemConfig` / `ActorSystemSetup` は `with_shared_factory(...)` や `with_lock_factory(...)` のような shared runtime override API を持たない。default dispatcher の seed、spawn path、bootstrap path は builtin spin backend を前提に直接 shared wrapper / state を組み立てる。

この判断に伴い、override seam の最後の名残である `ActorLockFactory` trait 自体も削除する。runtime-managed shared wrapper / state 構築は trait object ではなく direct builtin spin construction だけを使う。

pre-release で後方互換も不要な以上、使われていない一般化のために config surface を肥大化させる理由はない。

代替案:
- 互換のために deprecated API を残す: 今回は破壊的変更歓迎の前提なので不採用
- default config だけ direct construction にし、custom override seam は残す: 設計が二重化するため不採用

### 4. std adapter の `shared_factory` 公開面は削除する

`modules/actor-adaptor-std/src/std/system/shared_factory/`、`StdActorSharedFactory`、`DebugActorSharedFactory` は削除する。std adapter は actor runtime の lock family override surface を提供しない。

これにより std adapter 側にだけ残っていた duplicate concrete 実装を丸ごと落とせる。

代替案:
- std adapter にだけ debug/std factory を残す: core と std で構築方針が分岐するため不採用
- `StdActorLockFactory` のような rename で温存する: ゴール自体が direct construction なので不採用

### 5. generic `*SharedFactory` は actor runtime shared wrapper 構築に関わるものだけ削除する

`ActorFutureSharedFactory<AskResult>` のように actor runtime shared wrapper 構築の一部だったものは direct construction へ吸収して削除する。一方 `CircuitBreakerSharedFactory<C>` のような type-indexed registry capability は、`*SharedFactory` という名前でも単なる lock wrapper 構築戦略ではないため今回の対象外とする。

代替案:
- suffix が `SharedFactory` のものをすべて削除する: 別責務まで巻き込むため不採用
- generic `*SharedFactory` を全部残す: 今回の戦略転換が半端になるため不採用

### 6. テストは factory stub ではなく direct construction を前提に組み直す

現在の test double は `with_shared_factory(...)` と大量の trait 実装に引きずられている。今後は direct construction を前提に、wrapper 単体テストと wiring テストを builtin spin backend 固定で書く。

少なくとも現時点で `with_shared_factory(...)` の call site は 16 箇所あり、`std/system/shared_factory/tests.rs`、`actor_system_config/tests.rs`、`typed/system/tests.rs`、dispatcher tests 群が置換対象になる。

## Risks / Trade-offs

- [Risk] debug/std lock family 切替と same-thread 再入検知の導線が失われる → Mitigation: simplicity を優先する判断として受け入れ、将来必要になった時点で別 change として再導入する
- [Risk] direct constructor 呼び出しが散在して多少重複する → Mitigation: 型内または file-local helper までに留め、workspace-wide seam は作らない
- [Risk] `SharedRwLock` 利用箇所まで広げることで変更範囲が大きくなる → Mitigation: runtime-managed path を優先し、純ローカル utility 的箇所は必要なら follow-up に切る
- [Risk] completed change 群の設計意図と逆向きになる → Mitigation: pre-release / 破壊的変更歓迎 / Less is more を優先する再判断として proposal と specs に明記する

## Migration Plan

1. actor runtime の runtime-managed shared wrapper / shared state を direct builtin spin construction へ戻す
2. dispatcher configurator、spawn path、bootstrap、event stream helper、mailbox helper の wiring から `*SharedFactory` 依存を除去する
3. `ActorSystemConfig` / `ActorSystemSetup` の shared runtime override field / API を削除する
4. `modules/actor-adaptor-std/src/std/system/shared_factory/` と関連公開型を削除する
5. 旧 `*SharedFactory` trait / concrete 型 / tests / docs を削除し、direct construction 前提の test へ置換する

## Open Questions

- `serialization_registry` のような `SharedRwLock` 利用箇所のうち、actor runtime の core path と見なす範囲をどこまで今回に含めるか
