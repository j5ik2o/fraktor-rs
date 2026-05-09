## Context

現在の Mailbox scheduling state は `AtomicU32` / `AtomicBool` ベースで、実行権の獲得自体は lock-free に近い。一方で通常 user queue は `UnboundedMessageQueue` → `QueueStateHandle` → `SharedLock<QueueState<T>>` → `SyncQueueShared` → `SpinSyncMutex<SyncQueue<...>>` という経路を通り、さらに `Mailbox::enqueue_envelope` は close/cleanup race を避けるため通常 enqueue でも `put_lock` を取得している。

このため、単に queue 内部を lock-free にしても `put_lock` が残る限り hot path の直列化は消えない。本 change では、通常 unbounded user queue に queue-local atomic close protocol を持たせ、通常 enqueue path から `put_lock` を外すところまでを同じ scope とする。

## Goals / Non-Goals

**Goals:**

- 通常 unbounded mailbox user queue の enqueue/dequeue hot path から shared lock を除去する。
- 通常 enqueue path から `Mailbox::put_lock` を外し、close 後 enqueue rejection を queue-local atomic close protocol で保証する。
- `MessageQueue` trait、`Envelope` payload、`Mailbox::run` の drain semantics は維持する。
- unsafe は mailbox-local queue primitive に局所化し、公開 API は safe にする。
- producer/consumer interleaving、drop safety、FIFO、exact-once delivery をテストで固定する。

**Non-Goals:**

- bounded queue の lock-free 化。
- priority / stable-priority / control-aware queue の lock-free 化。
- deque/prepend-capable queue の lock-free 化。
- `SharedMessageQueue` / `BalancingDispatcher` の multi-consumer queue 置き換え。
- `utils-core` への汎用 MPSC primitive 昇格。
- ActorCell 排他制御や mailbox schedule state の構造整理。

## Decisions

### Decision 1: 初回スライスは mailbox-local primitive として実装する

lock-free MPSC は汎用化できる可能性があるが、初回は `actor-core-kernel::dispatch::mailbox` 配下の mailbox-local primitive として実装する。

理由:

- close rejection と cleanup drain は Mailbox semantics と密接に結びついている。
- unsafe の安全性契約を小さく保てる。
- `utils-core` に昇格すると、汎用 API、feature/cfg、複数 consumer 誤用対策、error 型の安定化まで同時に設計する必要がある。

代替案:

- `utils-core` に最初から置く: 再利用性は高いが scope が広がり、Mailbox hot path 改善の着手が遅くなるため採用しない。

### Decision 2: 対象は通常 unbounded FIFO queue のみ

`UnboundedMessageQueue` を lock-free MPSC-backed に置き換える。bounded / deque / priority / control-aware queue は既存実装を維持する。

理由:

- 通常 unbounded FIFO は最も基本的な user queue で、overflow / priority / prepend の複合 semantics を持たない。
- bounded overflow (`DropNewest` / `DropOldest`) や deque prepend は atomic queue だけでは完結せず、別の correctness problem になる。
- `SharedMessageQueue` は multi-consumer 前提であり、MPSC の single-consumer 契約と一致しない。

### Decision 3: close protocol は queue-local に持たせる

通常 lock-free queue は `closed` flag と in-flight producer count を持つ。enqueue は producer guard を取得してから close を再確認し、close が勝った場合は push せずに `Closed` を返す。close/cleanup は `closed = true` を publish し、in-flight producer count が 0 になるまで待ってから drain する。

概念的な流れ:

```text
offer(envelope)
  if closed => Err(Closed)
  producer_count += 1
  if closed => producer_count -= 1; Err(Closed)
  push node by CAS
  len += 1
  producer_count -= 1

close_and_drain()
  closed = true
  wait producer_count == 0
  drain pending/head lists
```

理由:

- `put_lock` なしでも「cleanup が close を宣言した後に enqueue が成功して残る」race を防げる。
- close は cold path なので、in-flight producer 待ちの spin は hot path に影響しない。
- producer guard 取得後は allocation や user callback を行わず、panic による count leak を避ける。

代替案:

- `Mailbox::put_lock` を残す: correctness は簡単だが、lock-free queue を導入しても hot path の直列化が残るため採用しない。
- close 後も push を許して cleanup が複数回 drain する: termination 境界が曖昧になり、post-close enqueue rejection contract と合わないため採用しない。

### Decision 4: consumer 側の safety guard を持つ

Mailbox state machine 上は通常 mailbox の runner は単一 consumer だが、`MessageQueue` trait は `&self` で `dequeue` を公開し、型だけでは concurrent dequeue を禁止できない。safe API で UB を起こさないため、primitive か wrapper は consumer-side guard を持ち、同時 dequeue / cleanup を直列化する。

理由:

- unsafe primitive の single-consumer 前提を safe trait の外へ漏らさない。
- 通常運用では mailbox runner が単一 consumer なので、この guard は競合しない。

### Decision 5: unsafe と検証の境界

unsafe は node allocation / raw pointer link / `Box::from_raw` に限定する。各 unsafe block には SAFETY comment を付け、通常 unit/stress tests に加えて `miri` と `loom` の検証タスクを持つ。

`loom` は atomic ordering と interleaving の検証に使う。`miri` は raw pointer ownership、double free、use-after-free、dangling pointer の検出に使う。

## Risks / Trade-offs

- [Risk] close protocol の producer count が漏れると cleanup が待ち続ける。  
  Mitigation: producer guard は RAII で decrement し、guard 取得後に user code / allocation を実行しない。

- [Risk] FIFO ordering が concurrent producer の CAS linearization とずれる。  
  Mitigation: CAS 成功順を enqueue の linearization point とし、consumer は swapped list を reverse して処理する。producer ごとの FIFO と exact-once を stress/loom tests で固定する。

- [Risk] `MessageQueue` trait の safe API と MPSC single-consumer 契約がずれる。  
  Mitigation: consumer-side guard を置き、concurrent dequeue が UB にならない構造にする。

- [Risk] bounded/deque/priority queue が残るため、すべての mailbox user queue から lock が消えるわけではない。  
  Mitigation: 初回 scope を通常 unbounded FIFO に限定し、他 queue は別 change で扱う。

- [Risk] `loom` の cfg/test wiring が過剰に複雑になる。  
  Mitigation: primitive 専用の小さな model tests に限定し、production code に loom-specific abstraction を広く漏らさない。

## Migration Plan

1. lock-free queue primitive を mailbox-local module として追加する。
2. `UnboundedMessageQueue` を新 primitive に差し替える。
3. 通常 unbounded queue の enqueue/cleanup path から `put_lock` 依存を外す。
4. 既存 mailbox tests を通し、close/rejection/drain behavior が変わっていないことを確認する。
5. queue 専用 stress tests、`miri`、`loom` を追加して検証する。

Rollback は `UnboundedMessageQueue` を既存 `QueueStateHandle` backing に戻し、通常 enqueue path の `put_lock` 分岐を復旧することで行う。

## Open Questions

- `loom` を Cargo dev-dependency として追加するか、既存の検証 tooling に隔離するか。
- producer count の待機は単純 spin で十分か、cold path でも backoff helper を使うか。
