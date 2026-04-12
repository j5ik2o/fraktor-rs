## Context

現在の actor runtime は `ActorSharedFactory` 1個に対して、`create_message_dispatcher_shared`、`create_executor_shared`、`create_actor_ref_sender_shared`、`create_event_stream_shared`、`create_actor_cell_state_shared`、`create_mailbox_shared_set` など多様な生成責務を集中させている。これは object-safe な system-scoped override を維持するという利点はある一方、Interface Segregation Principle を崩しており、特定 subsystem の差し替えや test double の作成を不必要に重くしている。

今回やることは単純で、`ActorSharedFactory` を個別 factory trait に分割し、使う側が必要な trait だけを受け取るように変えるだけである。依存集約 struct の新設や追加 abstraction は、この change では扱わない。

## Goals / Non-Goals

**Goals:**
- `ActorSharedFactory` を廃止し、1責務1Factory trait へ分割する
- object-safe な `dyn` 契約を維持したまま subsystem ごとの差し替え可能性を上げる
- dispatcher / actor-cell / event-stream / actor-ref などの wiring を個別 Port ベースへ移行する

**Non-Goals:**
- `ActorFutureShared<T>` など generic shared をこの change で system-scoped Port に含めること
- 実装型 (`BuiltinSpinSharedFactory` など) を Port 数に合わせて細分化すること
- actor-* 以外のモジュールに同じ Port 分割を強制すること

## Decisions

### 1. Port は「型ごと」ではなく「生成責務ごと」に分割する

単一 `ActorSharedFactory` は廃止し、少なくとも以下の個別 trait を導入する。

- `ExecutorSharedFactory`
- `MessageDispatcherSharedFactory`
- `SharedMessageQueueFactory`
- `ActorRefSenderSharedFactory`
- `ActorSharedLockFactory`
- `ActorCellStateSharedFactory`
- `ReceiveTimeoutStateSharedFactory`
- `MessageInvokerSharedFactory`
- `EventStreamSharedFactory`
- `EventStreamSubscriberSharedFactory`
- `MailboxSharedSetFactory`

各 trait のメソッド名は `create` に統一し、trait 名が生成対象を表現する。

代替案:
- `DispatcherSharedFactory` に `create_shared_message_queue` を含める: dispatch subsystem での grouping は自然だが、shared queue は `BalancingDispatcher` 専用の別責務なので不採用
- `ActorSharedFactory` を維持したまま subtrait を生やす: God Factory の中心を残し続けるため不採用

### 2. concrete 実装型は 1 型で複数 Port を実装してよい

Port は分割するが、`BuiltinSpinSharedFactory`、`DebugActorSharedFactory`、`StdActorSharedFactory` を Port 数に合わせて 11 個の実装型へ分解する必要はない。1 つの concrete 型が複数 Port trait を実装して構わない。

これにより、

- 契約は細かく保つ
- 実装の重複は増やさない
- system ごとの lock family 差し替えも維持する

という 3 点を両立できる。

### 3. 移行は dispatcher / actor-ref / event-stream / actor-cell の順に行う

Port 分割の影響が最も大きいのは wiring 境界なので、まず以下の順で移行する。

1. dispatcher configurator / executor factory / balancing queue の wiring を個別 Port 化
2. `ActorRef::with_system` と ask / sender 経路を `ActorRefSenderSharedFactory` へ移行
3. event stream / subscriber helper を `EventStream*SharedFactory` へ移行
4. `ActorCell::create` の runtime-owned state を actor-cell 系 Port 群へ移行
5. 最後に `ActorSharedFactory` を削除

この順なら compile break の爆発を抑えやすい。

## Risks / Trade-offs

- [Risk] Port 数が増えて constructor 変更が多くなる → Mitigation: 使う側は必要な trait だけを受け取り、余計な集約 abstraction は導入しない
- [Risk] 実装型が多数 trait を実装し、結局まとまったままに見える → Mitigation: 契約と実装を分けて評価し、interface segregation を優先する
- [Risk] rename 影響が広く、module path 変更まで含めると churn が大きい → Mitigation: 先に Port 契約を追加し、呼び出し側を段階移行してから旧 trait を削除する
- [Risk] generic shared まで同じ方針で巻き込みたくなる → Mitigation: 本 change では object-safe な actor runtime shared に限定し、generic shared は対象外と明記する

## Migration Plan

1. 個別 factory trait 群を追加する
2. builtin / debug / std 実装に個別 trait 実装を与える
3. dispatcher → actor-ref → event-stream → actor-cell の順で利用側を移行する
4. 旧 `ActorSharedFactory` と旧 naming を削除し、spec / test / docs を更新する
