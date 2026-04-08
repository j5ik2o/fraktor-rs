## Context

現状の `RuntimeMutex<T>` / `RuntimeRwLock<T>` は、依然として `SpinSyncMutex<T>` / `SpinSyncRwLock<T>` の alias であり、caller 側から lock 実装を差し替える seam が存在しない。

```text
RuntimeMutex<T>
  -> SpinSyncMutex<T>
     -> spin::Mutex<T>
```

このため、次の 2 つができない。

- std 環境で `std::sync::Mutex` ベースの通常 driver を選ぶ
- actor-core の再入 hot path に debug driver を差し込んで deadlock を検知する

actor-core ではすでに再入 deadlock を強く意識したコードが存在する。

- [`ActorRefSenderShared`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/actor/actor_ref/actor_ref_sender_shared.rs)
  - sender lock 解放後に schedule outcome を適用する
- [`DispatcherSender`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender.rs)
  - enqueue と schedule を 2 phase に分離する
- [`MessageDispatcherShared`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs)
  - inline executor 下の `mailbox.run(...)` 再入について warning を持つ
- [`ExecutorShared`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs)
  - trampoline を自前で持ち、inline executor 再入 deadlock を避ける

したがって Phase V の価値は「全部 genericize すること」ではなく、「再入 hot path へ driver seam を入れて観測可能にすること」にある。

## Goals / Non-Goals

**Goals**

- `RuntimeMutex` / `RuntimeRwLock` を driver 差し替え可能な port surface へ昇格する
- actor-core の再入 hot path に限定して factory genericization を導入する
- std adapter 側に `StdSyncMutex` と `DebugSpinSyncMutex` を追加する
- deadlock 検知 / driver 差し替えの価値を actor-core 単独で成立させる

**Non-Goals**

- workspace 全体の lock 利用箇所を一括 genericization すること
- cluster / persistence / stream を同時に移行すること
- Phase V-A で actor-core の全 shared wrapper を genericize すること
- 多数の driver 候補を一気に追加すること

## Decisions

### 1. Phase V は actor-core hot path を先に切る

最初の導入対象は actor-core の再入 hot path に絞る。選定基準は「出現回数」ではなく「再入 deadlock 検知として意味がある call chain 上にあるか」である。

#### Phase V-A: first-class hot path

- `ActorRefSenderShared`
- `MessageDispatcherShared`
- `ExecutorShared`
- `Mailbox`

この 4 つは以下の鎖に直接乗っている。

```text
tell
  -> ActorRefSenderShared
    -> DispatcherSender
      -> MessageDispatcherShared.dispatch_enqueue
        -> register_for_execution
          -> ExecutorShared.execute
            -> mailbox.run
              -> user handler
                -> re-entrant tell / ask / pipe_to_self
```

ここに seam を入れれば、deadlock 検知の価値は actor-core 単独で成立する。

ただし、実装上の変更がこの 4 型だけで完結するとは限らない。型パラメータの伝播により、少なくとも次の transitive caller は追従が必要になる可能性が高い。

- `DispatcherSender`
- `ActorCell`
- 関連 tests / examples

したがって Phase V-A の「4 つ」は実装ファイル数ではなく、設計判断の primary target を表す。

### 2. actor-core のその他 shared wrapper は Phase V-B に送る

次の型は lock seam 導入の候補だが、Phase V-A では後回しにする。

- `SharedMessageQueue`
- `EventStreamShared`
- `ActorRefProviderShared`
- `RemoteWatchHookShared`
- `SerializationExtensionShared`
- `SchedulerShared`

これらは shared wrapper として重要だが、send/schedule/run の再入 hot path からは一段離れている。`SharedMessageQueue` は BalancingDispatcher の shared lane として重要だが、Phase V-A の最短再入鎖の成立には必須ではない。

### 3. driver の配置は core builtin と std adapter に分ける

driver の責務分担は次のように固定する。

```text
utils-core
  LockDriver / RwLockDriver
  LockDriverFactory / RwLockDriverFactory
  RuntimeMutex / RuntimeRwLock
  SpinSyncMutex / SpinSyncRwLock
  SpinSyncFactory / SpinSyncRwLockFactory

utils-adaptor-std
  DebugSpinSyncMutex / DebugSpinSyncRwLock
  DebugSpinSyncFactory / DebugSpinSyncRwLockFactory
  StdSyncMutex / StdSyncRwLock
  StdSyncFactory / StdSyncRwLockFactory
```

理由:

- `SpinSyncMutex` は no_std builtin なので core 側に残す必要がある
- `DebugSpinSyncMutex` と `StdSyncMutex` は std adapter driver として配置するのが自然
- caller は mutex 実体より factory を選ぶ形に寄せるほうが API surface が安定する

### 4. `RuntimeMutex<T>` / `RuntimeRwLock<T>` は default-driver surface として維持する

`RuntimeMutex<T>` / `RuntimeRwLock<T>` は workspace 全体で使われているため、名前自体を消して caller を一括書き換えする方針は採らない。

この change で行うのは次である。

- 旧 `RuntimeMutex<T> = SpinSyncMutex<T>` / `RuntimeRwLock<T> = SpinSyncRwLock<T>` という alias 定義はやめる
- 代わりに、新しい port surface の default-driver instantiation として `RuntimeMutex<T>` / `RuntimeRwLock<T>` を提供する
- 既存 caller は `RuntimeMutex<T>` / `RuntimeRwLock<T>` を引き続き書ける
- hot path だけが factory override により debug/std driver を選べる

`NoStdMutex<T>` は `RuntimeMutex<T>` に追従する二次 alias として維持する。

### 5. 型パラメータ伝播は public API へ漏らさず、bootstrap/configurator 境界で driver family を固定する

型パラメータ伝播の上限は public API の手前で止める。

決定:

- `ActorSystem`, typed system, `ActorRef` など public 型は nongeneric のまま維持する
- driver family の選択は bootstrap / configurator 境界で 1 つ固定する
- Phase V-A の hot path wrapper は crate-internal に factory generic になってよい
- 必要なら `ActorCell` までは内部的に伝播してよい
- ただし `ActorSystem<D>` のように public surface へ driver parameter を漏らしてはならない

つまり、Phase V-A は full genericization ではなく internal genericization + public erasure/defaulting を採る。

### 6. LockDriver / RwLockDriver は GAT ベースの static-dispatch 契約を採る

driver 契約の最小方向性はここで固定する。

- guard 型は trait の associated type として表現する
- `lock()` / `read()` / `write()` は poison や driver 固有エラーを caller へ露出しない
- poison は driver 実装側で吸収する
- hot path では `dyn LockDriver` のような trait object erasure を前提にしない

要するに、Phase V-A の driver 契約は GAT ベースの static dispatch を前提にする。これにより `SpinSyncMutex` / `DebugSpinSyncMutex` / `StdSyncMutex` の 3 driver を同一 contract へ載せる。

### 7. `StdSyncMutex` は Phase V-A で入れる

今回は `StdSyncMutex` を先送りしない。Phase V-A の時点で `DebugSpinSyncMutex` と並べて std adapter driver として定義する。

これにより std 環境の caller は次の 3 種を比較できる。

- `SpinSyncMutex`
- `DebugSpinSyncMutex`
- `StdSyncMutex`

### 8. `StdSyncRwLock` も対称に入れるが、Phase V-A では caller 追従を要求しない

port surface を `Mutex` だけ先行させると設計が歪むので、`RwLock` も対称に定義する。ただし Phase V-A で追従させる actor-core hot path caller は、現時点では想定しない。

決定:

- `LockDriver` / `RwLockDriver` と factories は `Mutex` / `RwLock` 両方定義する
- `DebugSpinSyncRwLock` / `StdSyncRwLock` も同時に定義する
- Phase V-A では actor-core hot path の `RwLock` caller migration はゼロでもよい
- `RwLock` caller の concrete migration は Phase V-B 以降で扱う

### 9. poison policy は adapter 側で吸収し、caller へ露出させない

`StdSyncMutex` / `StdSyncRwLock` は `std::sync` 由来の poison を持つ。これは caller に露出させず、driver 実装側で吸収する。

この change で必要なのは poison policy の public contract ではなく、caller が lock 実装差を意識せず factory を選べることだからである。

具体的な unwrap/panic/recover 方針は実装時の詳細だが、少なくとも `LockDriver` 契約に poison を持ち込まないことを decision とする。

## Hot Path Inventory

### A. direct hot path targets

| 型 | 理由 | Phase |
|---|---|---|
| `ActorRefSenderShared` | per-actor sender lock。re-entrant tell の最前面 | A |
| `MessageDispatcherShared` | dispatcher write lock。enqueue/schedule の中核 | A |
| `ExecutorShared` | inline executor 再入を吸収する trampoline 所有 | A |
| `Mailbox` | `run()` と enqueue/prepend/cleanup の交点 | A |

### B. secondary actor-core wrappers

| 型 | 理由 | Phase |
|---|---|---|
| `SharedMessageQueue` | BalancingDispatcher の shared lane。最短再入鎖からは一段離れる | B |
| `EventStreamShared` | lock 外 callback 実行を持つが hot tell path ではない | B |
| `ActorRefProviderShared` | shared provider wrapper。頻度は高いが再入観測価値は A より低い | B |
| `RemoteWatchHookShared` | remote watch path 専用 | B |
| `SerializationExtensionShared` | extension wrapper | B |
| `SchedulerShared` | scheduler path は広いが Phase V-A の success に必須ではない | B |

### C. out of scope for first pass

| 領域 | 理由 |
|---|---|
| `SerializationRegistry` | `RuntimeRwLock` 多用だが hot tell path から遠い |
| `cluster-core` | crate boundary をまたぐ genericization が重い |
| `persistence-core` | persistence context 系は別の correctness 軸がある |
| `stream-core` | actor-core hot path 価値と独立 |

## Risks / Trade-offs

- `RuntimeMutex` seam を actor-core hot path へ入れるだけでも型パラメータ伝播は広がる
- `StdSyncMutex` / `StdSyncRwLock` の poison policy を雑に決めると driver contract が濁る
- `RwLock` を対称に入れることで初期実装量は増える
- A/B/C の境界が曖昧だと scope がすぐ膨らむ

## Open Questions

- `ActorCell` までの internal genericization を、型引数で素直に通すか、crate-internal alias で局所化するか
- `SharedMessageQueue` を本当に Phase V-B に留め切れるか。BalancingDispatcher の test/constructor 追従で結合が強いと分かった場合のみ Phase V-A へ繰り上げる

## Success Criteria

- actor-core hot path だけで factory genericization が成立する
- `DebugSpinSyncMutex` を actor-core hot path に差し込める
- `StdSyncMutex` を std adapter driver 候補として選べる
- cluster / persistence / stream を未移行でも Phase V-A 単独の価値が成立する
