# actor モジュール ギャップ分析

更新日: 2026-05-11

## 比較スコープ定義

この分析では、actor は旧 `actor-core` 内の kernel / typed サブディレクトリではなく、現行の分割済みクレートを parity 単位にする。

| 層 | fraktor-rs 側 | Pekko 側 | 扱い |
|----|---------------|----------|------|
| kernel | `modules/actor-core-kernel/src/` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/`, `references/pekko/actor/src/main/scala/org/apache/pekko/routing/`, `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/`, `references/pekko/actor/src/main/scala/org/apache/pekko/event/` | classic / untyped actor runtime 契約 |
| typed | `modules/actor-core-typed/src/` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` | typed actor API と typed runtime 契約。ただし Java DSL は除外 |
| std adaptor | `modules/actor-adaptor-std/src/` | Pekko の dispatcher / scheduler / logging 実装のうち Rust std adapter として意味を持つ契約 | tokio / thread / clock / tracing 等の adapter 実装 |

対象に含めるもの:

| 分類 | Pekko 側の主な根拠 | fraktor-rs 側の対応 |
|------|--------------------|---------------------|
| classic actor core | `Actor.scala`, `ActorRef.scala`, `ActorCell.scala`, `ActorSystem.scala`, `Props.scala`, `ActorPath.scala`, `ActorSelection.scala` | `modules/actor-core-kernel/src/actor/`, `modules/actor-core-kernel/src/system/` |
| supervision / lifecycle / DeathWatch | `FaultHandling.scala`, `dungeon/DeathWatch.scala`, `MessageAndSignals.scala` | `modules/actor-core-kernel/src/actor/supervision/`, `modules/actor-core-kernel/src/actor/actor_cell.rs`, `modules/actor-core-typed/src/message_and_signals/` |
| dispatch / mailbox | `dispatch/Mailbox.scala`, dispatcher abstractions | `modules/actor-core-kernel/src/dispatch/`, `modules/actor-adaptor-std/src/dispatch/` |
| routing | `routing/*.scala`, typed `scaladsl/Routers.scala`, typed `internal/routing/*.scala` | `modules/actor-core-kernel/src/routing/`, `modules/actor-core-typed/src/dsl/routing/` |
| event / logging | `event/EventStream.scala`, `event/Logging*.scala`, dead letters | `modules/actor-core-kernel/src/event/`, `modules/actor-adaptor-std/src/event/` |
| pattern | `pattern/AskSupport.scala`, `RetrySupport.scala`, `GracefulStopSupport.scala`, `CircuitBreaker.scala` | `modules/actor-core-kernel/src/pattern/`, `modules/actor-core-typed/src/dsl/ask_pattern.rs`, `modules/actor-adaptor-std/src/pattern/` |
| typed receptionist / pubsub / delivery | `typed/receptionist/Receptionist.scala`, `typed/pubsub/Topic.scala`, `typed/delivery/*.scala` | `modules/actor-core-typed/src/receptionist/`, `modules/actor-core-typed/src/pubsub/`, `modules/actor-core-typed/src/delivery/` |
| serialization / extension / shutdown | Pekko serialization contract, extension registry, `CoordinatedShutdown.scala` | `modules/actor-core-kernel/src/serialization/`, `modules/actor-core-kernel/src/actor/extension/`, `modules/actor-core-kernel/src/system/coordinated_shutdown*` |

対象外にするもの:

| 対象外 | 理由 |
|--------|------|
| Java DSL / Java interop: `AbstractActor`, `UntypedAbstractActor`, `ReceiveBuilder`, `javadsl/*`, `japi/*` | Java 継承 DSL / builder DSL であり、Rust では trait / builder / typed API に置き換える |
| Scala implicit / package ops / syntax sugar | Rust API として同型にする必要がない |
| JVM reflection / classloader / HOCON dynamic loading | JVM 実装方式に依存する |
| Java serialization / JFR / flight recorder | JVM 固有。Rust 側は serialization trait と tracing adapter を対象にする |
| deprecated classic remoting / Netty / Aeron 固有実装 | remote / transport の別スコープ |
| Pekko IO / TCP / UDP / DNS | actor core ではなく transport / network adapter の別スコープ。`modules/actor-core-kernel/src/io.rs` は private placeholder のため parity 分母に入れない |
| `*-tests`, `*-testkit`, `*-tck`, `src/test`, `multi-jvm` | ユーザーが testkit 調査を明示していないため除外 |

raw declaration count は参考値であり、parity 分母には使わない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 114 |
| fraktor-rs 固定スコープ対応概念 | 111 |
| 固定スコープ概念カバレッジ | 111/114 (97.4%) |
| raw Pekko public type declarations | 672 参考値（classic/routing/pattern/event: 415, typed: 257。Java DSL / JFR 等の除外前 raw） |
| raw Pekko public method declarations | 3008 参考値（classic/routing/pattern/event: 2036, typed: 972） |
| raw Rust public type declarations | 601 参考値（kernel: 450, typed: 133, std: 18） |
| raw Rust public method declarations | 2496 参考値（kernel: 1858, typed: 608, std: 30） |
| hard / medium / easy / trivial gap | 0 / 3 / 0 / 0 |
| `todo!()` / `unimplemented!()` / `panic!("not implemented")` | 0 件 |
| コメント上の TODO / placeholder | 4 件。うち parity ギャップ扱いは 3 件 |

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| kernel | classic actor core, DeathWatch, supervision, routing, event, pattern, serialization, shutdown | `fraktor-actor-core-kernel-rs` として独立。主要公開契約は到達可能 | 主要 API は十分にカバー。termination 完了経路と remote `AddressTerminated` 統合が残る |
| typed | typed ref/system/behavior/context, signal, router, receptionist, pubsub, delivery | `fraktor-actor-core-typed-rs` として独立し kernel に依存 | typed surface はほぼカバー。cluster receptionist 差分だけ部分実装 |
| std adaptor | executor, scheduler driver, std clock, tracing logging, circuit breaker registry | `fraktor-actor-adaptor-std-rs` に隔離 | core/std 境界は妥当 |

## カテゴリ別ギャップ

ギャップ（未対応・部分実装・n/a）のみテーブルに列挙する。実装済みはカテゴリの件数カウントに含める。

### classic actor core 実装済み 12/12 (100%)

該当ギャップなし。`Actor`, `ActorContext`, `ActorRef`, `ActorPath`, `ActorSelection`, `Props`, `ActorSystem`, address / deploy / child / spawn / stash 相当は kernel に存在する。

### supervision / lifecycle / DeathWatch 実装済み 8/10 (80%)

| Pekko API / 概念 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| parent termination completion (`finishTerminate`) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorCell.scala`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala` | 部分実装: `modules/actor-core-kernel/src/actor/actor_cell.rs:1163`, `modules/actor-core-kernel/src/actor/children_container.rs:188` | kernel | medium | `ChildrenContainer::remove_child_and_get_state_change` は `Termination` を返せるが、`handle_death_watch_notification` が `finish_terminate(pid)` へ接続していない |
| remote `AddressTerminated` DeathWatch integration | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:136`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/dungeon/DeathWatch.scala:218`, `references/pekko/actor/src/main/scala/org/apache/pekko/event/AddressTerminatedTopic.scala:34` | 部分実装: DeathWatch 基本経路と system message serializer はあるが remote / cluster address termination topic が未統合 | kernel + remote / cluster integration | medium | local DeathWatch は実装済み。remote / cluster の node termination を `Terminated` へ一度だけ翻訳する経路が残る |

### typed core surface 実装済み 12/12 (100%)

該当ギャップなし。`TypedActorRef`, `TypedActorSystem`, `Behavior`, `BehaviorInterceptor`, typed `ActorContext`, signals, typed props, dispatcher / mailbox selector, actor-ref resolver は `actor-core-typed` に分離済み。

### dispatch / mailbox 実装済み 10/10 (100%)

該当ギャップなし。core の `Executor`, `ExecutorFactory`, `Mailbox`, `MailboxType`, `MailboxRequirement`, `MailboxPolicy`, dispatcher registry と、std の tokio / threaded / pinned / affinity executor adapter が分離されている。

### routing 実装済み 11/11 (100%)

該当ギャップなし。classic routing は `RoundRobin`, `Random`, `ConsistentHashing`, `SmallestMailbox`, `Pool`, `Group`, `Routee`, `Router`, `RemoteRouter*` を kernel に持つ。typed routing は `Routers`, `PoolRouter`, `GroupRouter`, `TailChopping`, `ScatterGatherFirstCompleted`, `BalancingPool`, `Resizer` を typed に持つ。

### event / logging 実装済み 8/8 (100%)

該当ギャップなし。`EventStream`, subscriber, dead letter, unhandled message, lifecycle/remoting event, logging adapter, logger subscriber, tracing subscriber adapter が確認できる。

### pattern 実装済み 5/5 (100%)

該当ギャップなし。classic `ask`, timeout completion, `retry`, `graceful_stop`, `CircuitBreaker` と typed `AskPattern` が存在する。std registry は `modules/actor-adaptor-std/src/pattern/` に分離されている。

### receptionist / discovery 実装済み 4/5 (80%)

| Pekko API / 概念 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `Listing.servicesWereAddedOrRemoved` の cluster reachability 差分 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:425`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/internal/receptionist/ReceptionistMessages.scala:75` | 部分実装: `modules/actor-core-typed/src/receptionist/listing.rs:108` | actor-core-typed + cluster integration | medium | 現在は local-only として常に `true`。clustered receptionist reachability が入ると add/remove 差分を持つ必要がある |

### scheduling / timers 実装済み 5/5 (100%)

該当ギャップなし。kernel `Scheduler`, cancellable registry, receive timeout, classic timer scheduler と typed `TimerScheduler` が存在する。`modules/actor-core-kernel/src/actor/scheduler/scheduler_core.rs:1` は doc 上 placeholder と書かれているが、実体は timer wheel ベース実装を持つためギャップにはしない。

### ref / resolution 実装済み 4/4 (100%)

該当ギャップなし。classic `Identify` / `ActorIdentity`, actor path parser/formatter, actor selection resolver, typed `ActorRefResolver` / setup が存在する。

### delivery / pubsub 実装済み 9/9 (100%)

該当ギャップなし。`ProducerController`, `ConsumerController`, `WorkPullingProducerController`, durable queue, sequence number, confirmation qualifier, message sent state, typed `Topic`, `TopicCommand` が `actor-core-typed` に存在する。

### serialization contract 実装済み 9/9 (100%)

該当ギャップなし。`Serializer`, `SerializerWithStringManifest`, `ByteBufferSerializer`, async serializer, registry, extension, setup builder, serialized message, transport information が kernel に存在する。Java serialization そのものは対象外。

### extension 実装済み 3/3 (100%)

該当ギャップなし。kernel extension trait / id / installer と typed `ExtensionSetup`, typed receptionist extension が存在する。

### coordinated shutdown 実装済み 5/5 (100%)

該当ギャップなし。`CoordinatedShutdown`, phase, reason, installer, error が kernel に存在する。

### std adaptor 実装済み 6/6 (100%)

該当ギャップなし。`TokioExecutor`, `ThreadedExecutor`, `PinnedExecutor`, `AffinityExecutor`, `TokioTickDriver`, `StdClock`, tracing logger subscriber が std adaptor にある。

## 内部モジュール構造ギャップ

API ギャップは 3/114 と少ないため、内部構造も確認した。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| termination completion の集約点不足 | `ActorCell.finishTerminate` / `FaultHandling.handleChildTerminated` 相当 | `actor_cell.rs` と `children_container.rs` に `Termination` 遷移の受け皿はあるが、`finish_terminate` 呼び出しが未配線 | `ActorCell` 内に termination 完了処理を集約し、`SuspendReason::Termination` 返却時に呼ぶ | medium | high | local lifecycle parity の残り。remote / cluster より先に閉じられる |
| remote address termination event の経路不足 | `AddressTerminatedTopic`, `DeathWatch.addressTerminated`, remote / cluster watcher | kernel event stream, remoting lifecycle event, system message serializer はあるが、remote / cluster から DeathWatch へ流す topic / hook が未完成 | kernel に address-terminated 契約を置き、remote / cluster が publish する adapter を接続する | medium | medium | remote / cluster の責務確定後に接続するのが自然 |
| typed receptionist reachability diff の保持不足 | typed `Receptionist.Listing.servicesWereAddedOrRemoved` | `Listing` は local-only として常に `true` を返す | `Listing` に add/remove diff または差分フラグを保持し、clustered receptionist 実装から設定する | medium | medium | cluster receptionist 導入時の typed API セマンティクス差分 |
| kernel public surface の広さ | Pekko は public API と `private[pekko]` internal を明確に分ける | `modules/actor-core-kernel/src/actor.rs` が `ActorCell`, `ActorShared`, `ChildRef` など低レベル型を public re-export している | 外部契約として必要な型と `pub(crate)` / internal に落とせる型を棚卸しする | medium | low | parity 不足ではなく保守性改善。pre-release なので破壊的整理は可能 |

## 実装優先度

### Phase 1: trivial / easy

該当なし。

### Phase 2: medium

- parent termination completion (`finish_terminate`) を `SuspendReason::Termination` 経路へ配線する（kernel）
- remote `AddressTerminated` を DeathWatch へ統合する（kernel + remote / cluster integration）
- `Listing.services_were_added_or_removed` に cluster reachability の add/remove 差分を反映する（actor-core-typed + cluster integration）

### Phase 3: hard

該当なし。

### 対象外（n/a）

- Java DSL / `javadsl/*` / `japi/*`
- Scala implicit / package ops / syntax sugar
- JVM reflection / classloader / HOCON loading
- Java serialization / JFR / flight recorder
- deprecated classic remoting / Netty / Aeron 固有実装
- Pekko IO / TCP / UDP / DNS
- testkit / TCK / tests

## まとめ

actor モジュールは、分割後の `actor-core-kernel` / `actor-core-typed` / `actor-adaptor-std` を基準に見ても、公開 API parity は 111/114 (97.4%) まで到達している。今回の主な修正点は、旧 `actor-core` 内部サブディレクトリ前提を捨て、現行クレート分割をスコープ定義と層別カバレッジに反映したこと。

低コストで parity を前進できる代表は `finish_terminate` 配線と typed receptionist の差分フラグ保持である。主要ギャップは remote / cluster 連携後の `AddressTerminated` DeathWatch 統合で、これは actor 単体というより remote / cluster integration の境界タスクになる。

次のボトルネックは公開 API 追加ではなく、termination / reachability の内部イベント経路と kernel public surface の整理にある。
