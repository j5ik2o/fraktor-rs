# actor モジュール ギャップ分析

## 前提と集計範囲

- 比較対象:
  - fraktor-rs: `modules/actor/src/core/kernel`, `modules/actor/src/core/typed`, `modules/actor/src/std`
  - Pekko classic: `references/pekko/actor/src/main/scala/org/apache/pekko/actor`
  - Pekko typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed`, `.../scaladsl`, `.../receptionist`, `.../pubsub`, `.../delivery`
- 公開型数は宣言ベースの生 count:
  - Pekko classic: 160
  - Pekko typed: 120
  - fraktor-rs kernel: 291
  - fraktor-rs typed: 84
  - fraktor-rs std: 19
- ただし parity 判定は companion object や内包 case class の水増しを避けるため、以下の「契約 family」単位で集約する。
- この文書では YAGNI を適用しない。表に載せるのは「今の要求で実装するかどうか」ではなく、「Pekko parity として差が残っているかどうか」で判定する。
- 数値は概算を含むが、下のギャップ表は網羅を優先している。評価の正本は各カテゴリの行である。
- カバー率の分母は `n/a` を除いた「実装対象 parity family」とする。`n/a` を含めた総 universe は参考値として別記する。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 280（classic: 160, typed: 120） |
| fraktor-rs 公開型数 | 394（core/kernel: 291, core/typed: 84, std: 19） |
| カバレッジ（実装対象 parity family 単位） | 30/50 (60%) |
| ギャップ数 | 23（要対応: 20, n/a: 3） |

注記:

- このサマリー表の数値は初期分析時点の集計である
- 2026-04-03 時点の再検証結果の正本は、下の「再検証結果」と「実装優先度」である
- Phase 1 / Phase 2 の多くはこの初期集計後に解消されている

## 再検証結果（2026-04-03）

- `Phase 1`: 完了
- `Phase 2`: 一部完了。capability parity の主要部分は完了したが、medium 難易度の family が残っている
- `Phase 3`: 未完。classic parity の中核 family と typed 補助 surface が残っている

2026-04-03 時点で完了確認できた主な項目:

- `ActorRefResolverSetup`
- `TimerScheduler` の message-as-key shorthand
- `Receptionist.Listing.all_service_instances` / `services_were_added_or_removed`
- `ActorSystem.receptionist`
- `ActorSystem.event_stream`
- `ActorSystem.dead_letters`
- `ActorSystem.ignore_ref`
- `ActorSystem.print_tree`
- `ActorSystem.system_actor_of`
- classic `Timers` surface
- classic `Stash` capability parity
- `BehaviorSignal::PostStop`
- classic logging adapter family
- `TypedProps::empty()`

この再検証時点で残っている要対応 parity family:

- `MailboxSelector.unbounded`
- `ActorSelection` surface
- classic auto-received / monitoring message family
- `Deployer` / `Deploy` / `Scope`
- `ActorSystemSetup` / `BootstrapSetup`
- classic `FSM` / `AbstractFSM` / `LoggingFSM`
- classic fully-surfaced `ActorRefProvider` contract
- classic routing config hierarchy
- classic `Tcp` / `Udp` / `Dns` / `IO` extension family
- classic event / logging bus utility family の未充足分
- `Topic.TopicStats` の cluster-wide semantics
- `DurableProducerQueue` の独立 entry point

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 19 | 9 | 47% |
| core / typed ラッパー | 24 | 17 | 71% |
| std / アダプタ | 5 | 4 | 80% |

## Phase 対応時の想定カバー率

前提:

- 分母は `n/a` を除いた 50 family
- 現在の実装済みは 30 family
- Phase の項目は「その family を完全に埋める」と仮定して見積もる

| 時点 | 実装済み family | 想定カバー率 |
|------|-----------------|-------------|
| 現状 | 30/50 | 60% |
| Phase 1 完了後 | 35/50 | 70% |
| Phase 2 完了後 | 43/50 | 86% |
| Phase 3 完了後 | 50/50 | 100% |

参考:

- `n/a` を含めた総 universe は 53 family
- その見方だと最終到達点は `50/53 = 94.3%`
- ただし `n/a` は JVM/Java 固有で埋めない前提なので、進捗指標としては `50` を分母にする方が妥当

## カテゴリ別ギャップ

### classic コア actor 契約　✅ 実装済み 8/13 (62%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorSelection` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSelection.scala:39` | 部分実装 | core/kernel | hard | fraktor-rs は `ActorSelectionResolver` による相対パス解決のみ (`modules/actor/src/core/kernel/actor/actor_selection/resolver.rs:15`) で、selection handle 本体の `tell` / `forward` / `resolveOne` / `toSerializationFormat` がない |
| classic auto-received / monitoring message family (`ReceiveTimeout`, `UnhandledMessage`) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:147`, `:298` | 部分実装 | core/kernel | medium | `PoisonPill` / `Kill` / `Identify` / `ActorIdentity` は概ねあるが、公開 `ReceiveTimeout` 型と classic `UnhandledMessage` payload は未整備。fraktor-rs は `set_receive_timeout` API (`modules/actor/src/core/kernel/actor/actor_context.rs:528`) と typed 向け `TypedUnhandledMessageEvent` (`modules/actor/src/core/kernel/event/stream/typed_unhandled_message_event.rs:14`) に寄っている |
| `AbstractActor` / `UntypedAbstractActor` / `AbstractActorWith*` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/AbstractActor.scala:231`, `:338`, `:421` | 未対応 | n/a | n/a | Java 継承ベース DSL。fraktor-rs は `Actor` trait ベース (`modules/actor/src/core/kernel/actor/actor_lifecycle.rs:14`) のため同名 parity は対象外 |
| `Deployer` / `Deploy` / `Scope` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Deployer.scala:28`, `:68`, `:173` | 未対応 | core/kernel | medium | fraktor-rs の `Props` (`modules/actor/src/core/kernel/actor/props/base.rs:18`) は dispatcher / mailbox / tag に留まり、deploy scope や router deployment 設定を持たない |
| `ActorSystemSetup` / `BootstrapSetup` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSystem.scala:48`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/setup/ActorSystemSetup.scala:30` | 部分実装 | core/kernel | medium | fraktor-rs には `ActorSystemConfig` (`modules/actor/src/core/kernel/actor/setup/actor_system_config.rs:27`) はあるが、Pekko の setup object 合成モデルとは別設計 |
| `IndirectActorProducer` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/IndirectActorProducer.scala:29` | 未対応 | n/a | n/a | JVM の reflection / constructor indirection 前提。Rust の `Props::from_fn` (`modules/actor/src/core/kernel/actor/props/base.rs:54`) とは別物 |

### classic 補助 DSL　✅ 実装済み 1/6 (17%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Timers` / classic `TimerScheduler` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Timers.scala:31`, `:101` | 未対応 | core/kernel | medium | typed 側には `TimerScheduler` (`modules/actor/src/core/typed/dsl/timer_scheduler.rs:23`) があるが、classic actor へ mixin される `timers` surface はない |
| `Stash` / `UnboundedStash` / `UnrestrictedStash` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Stash.scala:71`, `:76`, `:82`, `:283` | 部分実装 | core/kernel | medium | `ActorContext::stash` / `unstash` (`modules/actor/src/core/kernel/actor/actor_context.rs:107`, `:142`) はあるが、actor trait に付与する stash 契約と `StashOverflowException` 相当の公開型がない |
| `FSM` / `AbstractFSM` / `LoggingFSM` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:29`, `:430`, `:937`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/AbstractFSM.scala:43` | 部分実装 | core/kernel | hard | fraktor-rs は typed 側に `FsmBuilder` (`modules/actor/src/core/typed/dsl/fsm_builder.rs:18`) を持つが、classic FSM の state timeout / transition subscription / `when` DSL は未実装 |
| classic logging adapter family (`ActorLogging`, `DiagnosticActorLogging`, `ActorLogMarker`, `LoggingReceive`) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:341`, `:371`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorLogMarker.scala:26` | 未対応 | std | medium | fraktor-rs は `LogEvent` / `LoggerSubscriber` / `TracingLoggerSubscriber` (`modules/actor/src/core/kernel/event/logging/log_event.rs:14`, `modules/actor/src/std/event/logging/tracing_logger_subscriber.rs:22`) はあるが、classic mixin / adapter surface は持たない |
| `DynamicAccess` / `ReflectiveDynamicAccess` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/DynamicAccess.scala:33`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ReflectiveDynamicAccess.scala:35` | 未対応 | n/a | n/a | JVM classloader / reflection 前提であり、Rust/no_std parity の対象外 |

### classic runtime / 拡張 family　✅ 実装済み 3/8 (38%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorRefProvider` の classic fully-surfaced contract | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRefProvider.scala:189` | 部分実装 | core/kernel | medium | fraktor-rs の `ActorRefProvider` は `supported_schemes` / `actor_ref` に絞っている (`modules/actor/src/core/kernel/actor/actor_ref_provider/base.rs:17`)。dead letters / guardian refs / temp path は future extension のまま |
| `Tcp` / `Udp` / `Dns` / `IO` extension family | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Timers.scala:31`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Address.scala:120` | 未対応 | std | hard | `modules/actor/src` には Pekko classic `io` 相当の公開 surface がない。`fraktor.tcp` scheme は actor path へ現れている (`modules/actor/src/core/kernel/actor/actor_path/actor_path_scheme.rs:9`) が、socket/DNS actor API は未着手 |
| classic routing config family (`RouterConfig`, `Pool`, `Group`, `FromConfig`, `BroadcastPool`, `RoundRobinPool`, `RandomPool`, `TailChoppingPool`, `SmallestMailboxPool` など) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:385`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Props.scala:124` | 部分実装 | core/kernel | hard | fraktor-rs には kernel `Router` / `Routee` / `RoutingLogic` (`modules/actor/src/core/kernel/routing/router.rs:22`, `routee.rs:15`) と typed router builder はあるが、classic `RouterConfig` 階層はない |
| classic event / logging bus utility family (`LoggingAdapter`, `BusLogging`, `NoLogging`, `LogMarker`) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:341`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorLogMarker.scala:26` | 部分実装 | std | medium | event stream と log event 本体はあるが、classic の utility wrapper / adapter 契約はない |
| JVM serialization extras (`JavaSerializer`, `CurrentSystem`, Java-compat serializer hierarchy) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:44` | 部分実装 | n/a | n/a | fraktor-rs は `SerializationSetup`, `Serializer`, `AsyncSerializer`, `ByteBufferSerializer`, `SerializerWithStringManifest` (`modules/actor/src/core/kernel/serialization.rs:30`) を揃える一方、JVM 依存の Java serializer family は対象外 |

### typed system / runtime　✅ 実装済み 10/17 (59%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorSystem.receptionist` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:181` | 部分実装 | core/typed | easy | fraktor-rs は `receptionist_ref()` が `Option` を返す (`modules/actor/src/core/typed/system.rs:118`)。Pekko のように常在 actor ref 契約ではない |
| `ActorSystem.eventStream` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:186` | 部分実装 | core/kernel | medium | `EventStreamCommand` 型自体はある (`modules/actor/src/core/typed/eventstream/event_stream_command.rs:8`) が、system API は `EventStreamShared` を返し、`ActorRef[EventStream.Command]` 契約ではない (`modules/actor/src/core/typed/system.rs:124`) |
| `ActorSystem.deadLetters` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:154` | 部分実装 | core/kernel | medium | Pekko は dead letter sink `ActorRef[U]`、fraktor-rs は記録済み `Vec<DeadLetterEntry>` (`modules/actor/src/core/typed/system.rs:142`) |
| `ActorSystem.ignoreRef` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:159` | 未対応 | core/typed | easy | no-op recipient ref は公開されていない |
| `ActorSystem.printTree` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:167` | 未対応 | core/kernel | medium | actor hierarchy dump surface がない |
| `ActorSystem.systemActorOf` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:176` | 部分実装 | core/typed | medium | kernel には `system_actor_of` が private である (`modules/actor/src/core/kernel/system/base.rs:438`) が typed public API に出ていない |
| `ActorRefResolverSetup` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRefResolver.scala:95` | 未対応 | core/typed | easy | `ActorRefResolver` 本体 (`modules/actor/src/core/typed/actor_ref_resolver.rs:22`) はあるが setup hook がない |
| `AskPattern` object / extension-method surface | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/AskPattern.scala:38` | 部分実装 | core/typed | easy | `TypedActorRef::ask` と `TypedActorContext::ask` はある (`modules/actor/src/core/typed/actor_ref.rs:89`, `modules/actor/src/core/typed/actor/actor_context.rs:510`) が、独立した pattern surface はない |

### typed 振る舞い / シグナル / 設定 DSL　✅ 実装済み 8/12 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Signal` family の契約一致 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:35`, `:43`, `:59`, `:81`, `:104`, `:125` | 部分実装 | core/typed | medium | fraktor-rs は `BehaviorSignal` (`modules/actor/src/core/typed/message_and_signals/signal.rs:10`) で `Terminated` / `ChildFailed` / `MessageAdaptionFailure` / `PreRestart` は持つが、`PostStop` を `Stopped` に置き換え、さらに Pekko にない `Started` を追加しており契約が一致していない |
| `MailboxSelector.unbounded` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:208`, `:224` | 未対応 | core/typed | easy | fraktor-rs の `MailboxSelector` は `Default` / `Bounded` / `FromConfig` のみ (`modules/actor/src/core/typed/mailbox_selector.rs:13`) |
| `TimerScheduler` の message-as-key convenience overload | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/TimerScheduler.scala:87`, `:107`, `:243` | 部分実装 | core/typed | trivial | fraktor-rs の `TimerScheduler` は key 必須 (`modules/actor/src/core/typed/dsl/timer_scheduler.rs:48`, `:117`)。`msg` 自体を key とみなす shorthand がない |
| typed `Props.empty` / linked-list style props contract | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:26`, `:41` | 部分実装 | core/typed | medium | fraktor-rs は `TypedProps::from_props` / `with_*` はある (`modules/actor/src/core/typed/props.rs:58`) が、`Props.empty` 相当の空 props と linked-list 合成モデルは公開していない |

### typed ディスカバリ / pubsub / delivery　✅ 実装済み 9/12 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receptionist.Listing.allServiceInstances` / `servicesWereAddedOrRemoved` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:402`, `:425` | 部分実装 | core/typed | easy | fraktor-rs の `Listing` は `service_instances` / `is_for_key` まで (`modules/actor/src/core/typed/receptionist/listing.rs:73`) |
| `Topic.TopicStats` 契約の cluster-wide semantics | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/pubsub/Topic.scala:90` | 部分実装 | core/typed | medium | fraktor-rs の `TopicStats` は local subscriber / topic instance count を返す (`modules/actor/src/core/typed/pubsub/topic.rs:113`) が、Pekko distributed pubsub 前提の cluster-wide semantics までは持たない |
| `WorkPullingProducerController` / `ProducerController` の durable queue を actor protocol として単体公開する family | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/DurableProducerQueue.scala:33` | 部分実装 | core/typed | medium | fraktor-rs は `DurableProducerQueueCommand` / `DurableProducerQueueState` と `behavior_with_durable_queue` を持つ (`modules/actor/src/core/typed/delivery/durable_producer_queue_command.rs:18`, `producer_controller.rs:165`, `work_pulling_producer_controller.rs:244`) が、Pekko の `DurableProducerQueue` object/trait として独立した entry point はない |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。

判定理由:
- `hard` / `medium` の未実装ギャップが 11 件あり、しきい値 5 件を超えている
- classic 側で `ActorSelection` / `FSM` / routing config / io family / deploy-setup family / `ActorRefProvider` fully-surfaced contract が未充足
- typed 側でも standalone `AskPattern` / `TopicStats` cluster semantics / `DurableProducerQueue` entry point / `MailboxSelector.unbounded` が残っている

したがって、次のボトルネックは内部責務分割ではなく公開 API parity である。

## 実装優先度

この節の記法:

- `未対応` の項目は「追加する」と書く
- `部分実装` の項目は「不足している何を埋めるか」を併記する
- したがって、各 Phase の項目は直前のギャップ表の再配置であり、新しい提案ではない
- ただし、このファイル上部のカテゴリ別ギャップ表とサマリー表は初期分析時点のスナップショットである。2026-04-03 時点の再検証と Phase 判定の正本は、この節の記述を正とする

### Phase 1

状態: 完了

この Phase で完了済みの項目:

- 完了: `ActorRefResolverSetup` を追加済み（core/typed）
- 完了: `MailboxSelector.unbounded` を追加済み（core/typed）
- 完了: `TimerScheduler` の message-as-key shorthand を追加済み（core/typed）
- 完了: `Receptionist.Listing` に `allServiceInstances` と `servicesWereAddedOrRemoved` を追加済み（core/typed）
- 完了: `ActorSystem.receptionist` は常在 actor ref 契約を `receptionist()` で満たし、互換補助として `receptionist_ref()` も保持している（core/typed）
- 完了: standalone `AskPattern` surface を `AskPattern::ask` / `AskPattern::ask_with_status` として追加済み（core/typed）

### Phase 2

状態: 一部完了

この Phase の完了条件は、Pekko 名称の直訳ではなく、fraktor-rs の公開契約として必要な capability parity を満たすこととする。

特に classic stash については、`Stash` / `UnboundedStash` / `UnrestrictedStash` という trait 名を Rust へそのまま移植することを要求しない。代わりに、以下を満たせば完了とみなす。

- actor が `stash` / `unstash` / `unstash_all` を公開 API として利用できる
- stash 対応 mailbox 要件を `MailboxRequirement::for_stash()` で宣言できる
- overflow を `StashOverflowError` で公開エラーとして扱える

Phase 2 の各項目は以下の状態で完了している。

- 完了: `ActorSystem.eventStream` は `TypedActorRef<EventStreamCommand>` 契約へ更新済み
- 完了: `ActorSystem.deadLetters` は dead letter sink `ActorRef` 契約へ更新済み
- 完了: `ActorSystem.ignoreRef`、`printTree`、`system_actor_of` は typed public API として公開済み
- 完了: classic `Timers` surface は `ActorContext::timers()` + `ClassicTimerScheduler` として公開済み
- 完了: classic `Stash` は capability parity で完了。`ActorContext::stash` / `stash_with_limit` / `unstash` / `unstash_all`、`MailboxRequirement::for_stash()`、`StashOverflowError` を公開している
- 完了: `BehaviorSignal` は `PostStop` を含む契約へ更新済み
- 完了: classic logging adapter family は `ActorLogMarker` / `ActorLogging` / `DiagnosticActorLogging` / `LoggingAdapter` / `LoggingReceive` を公開済み
- 完了: `TypedProps::empty()` を追加済み

この Phase の残件:

- classic auto-received / monitoring message family の不足分を埋める。`ReceiveTimeout` と classic `UnhandledMessage` payload を公開 surface として揃える（core/kernel）
- `Deployer` / `Deploy` / `Scope` を追加する。現在の `Props` は dispatcher / mailbox / tag に留まり、deploy scope や router deployment 設定を持たない（core/kernel）
- `ActorSystemSetup` / `BootstrapSetup` の不足分を埋める。現在は `ActorSystemConfig` があるが、Pekko の setup object 合成モデルとは別設計のままである（core/kernel）
- `ActorRefProvider` の不足分を埋める。現在は `supported_schemes` / `actor_ref` だけなので、guardian refs、dead letters、temp path などの classic surface を拡張する（core/kernel）
- classic event / logging bus utility family の不足分を埋める。現在の classic logging adapter family に加えて、`BusLogging`、`NoLogging` などの utility wrapper parity を揃える（std）
- `Topic.TopicStats` の不足分を埋める。現在の local 集計から、Pekko pubsub が前提にする cluster-wide semantics へ寄せる（core/typed）
- `DurableProducerQueue` の独立 entry point を追加する。現在は command/state 型と `behavior_with_durable_queue` はあるが、Pekko の `DurableProducerQueue` object/trait 相当の公開入口がない（core/typed）

### Phase 3

- `ActorSelection` の不足分を埋める。現在の `ActorSelectionResolver` に加えて、selection handle 本体、`tell`、`forward`、`resolveOne`、`toSerializationFormat` を実装する（core/kernel）
- classic `FSM` / `AbstractFSM` parity の不足分を埋める。typed `FsmBuilder` とは別に、classic の state timeout、transition subscription、`when` DSL、`LoggingFSM` を実装する（core/kernel）
- classic routing config hierarchy の不足分を埋める。現在の kernel `Router` / `Routee` / `RoutingLogic` に加えて、`RouterConfig`、`Pool`、`Group`、`FromConfig` などの公開設定 surface を実装する（core/kernel）
- classic `Tcp` / `Udp` / `Dns` / `IO` extension family を追加する。現在は actor path に `fraktor.tcp` scheme があるだけで、公開 actor API がない（std）

### 対象外（n/a）

- `AbstractActor*` Java DSL
- `DynamicAccess` / `ReflectiveDynamicAccess`
- `IndirectActorProducer`
- JVM serialization extras (`JavaSerializer`, `CurrentSystem`, Java-compat serializer hierarchy)

理由:
- いずれも JVM / Java 継承 / reflection / classloader 前提のため、Rust/no_std parity の直接対象ではない

## まとめ

- typed 側の主要 API はかなり揃っており、未完は `TopicStats` cluster semantics、`DurableProducerQueue` entry point など補助 family が中心である。
- classic 側は `Timers`、stash capability parity、classic logging adapter family までは埋まったが、`ActorSelection`、classic auto-received message、`FSM`、routing config、`io` family、fully-surfaced `ActorRefProvider` が主要ギャップとして残る。
- parity を低コストで前進できる easy ギャップは概ね埋まり、残件は medium 以上が中心である。
- parity 上の主要ギャップは classic の selection / deployment / routing / io / FSM family と、typed の cluster-aware discovery / delivery family である。
- API ギャップはまだ支配的であり、次のボトルネックは内部構造ではなく公開契約 parity の継続解消にある。
