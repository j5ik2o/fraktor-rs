# actor モジュール ギャップ分析

集計注記:
- 集計対象は `actor-typed` の typed root / `scaladsl` / `receptionist` / `eventstream` / `delivery` / `routing` と、typed API が参照する classic bridge (`ActorPath`, `Address`, `Cancellable`)。
- `javadsl` 重複、`internal` 実装、deprecated、JVM 専用 util は除外した。
- `fraktor-rs` 側の件数は「Pekko parity surface に対応する公開契約」の数であり、crate 全体の公開型総数ではない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 56 |
| fraktor-rs parity 実装数 | 40（core: 36, std: 4） |
| カバレッジ（型単位） | 40/56 (71%) |
| ギャップ数 | 16（core: 16, std: 0） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 6 | 6 | 100% |
| core / typed ラッパー | 46 | 30 | 65% |
| std / アダプタ | 4 | 4 | 100% |

## カテゴリ別ギャップ

### 基盤 DSL / 型　✅ 実装済み 15/22 (68%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ExtensibleBehavior[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Behavior.scala:106` | 未対応 | core/typed | hard | `AbstractBehavior` はあるが、`receive/receiveSignal` を持つ独立拡張点がない。 |
| `ActorRef.!` / `RecipientRef.!` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:77`, `:143` | 未対応 | core/typed | trivial | `tell` はある (`modules/actor/src/core/typed/actor_ref.rs:61`) が演算子エイリアスがない。 |
| `BehaviorInterceptor.isSame` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/BehaviorInterceptor.scala:92` | 未対応 | core/typed | trivial | fraktor の `BehaviorInterceptor` は `around_start/around_receive/around_signal` のみ (`modules/actor/src/core/typed/behavior_interceptor.rs:21`)。 |
| `Props.withMailboxFromConfig` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:90` | 別経路のみ | core/typed | trivial | `MailboxSelector::from_config` (`modules/actor/src/core/typed/mailbox_selector.rs:40`) と `TypedProps::with_mailbox_selector` (`modules/actor/src/core/typed/props.rs:111`) はあるが、shorthand がない。 |
| `SupervisorStrategy.resume/restart/stop` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:35`, `:44`, `:50` | 部分実装 | core/typed | medium | kernel の `SupervisorStrategy` はある (`modules/actor/src/core/kernel/actor/supervision/base.rs:23`) が、typed ルートの定数ファクトリがない。 |
| `RestartSupervisorStrategy` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:266` | 部分実装 | core/typed | medium | `with_stop_children` / `with_stash_capacity` は kernel 側にある (`modules/actor/src/core/kernel/actor/supervision/base.rs:186`, `:193`) が、typed 専用型と `withLimit` がない。 |
| `BackoffSupervisorStrategy` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:320` | 部分実装 | core/typed | medium | `BackoffSupervisorStrategy` 自体はある (`modules/actor/src/core/kernel/actor/supervision/backoff_supervisor_strategy.rs:18`) が、typed façade と Pekko と同じ builder surface ではない。 |

### ライフサイクル / Signals　✅ 実装済み 3/7 (43%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Signal` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:33` | 未対応 | core/typed | medium | fraktor は `BehaviorSignal` enum で一括表現しており、marker trait がない (`modules/actor/src/core/typed/message_and_signals/signal.rs:10`)。 |
| `PreRestart` / `PostStop` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:42`, `:52` | 部分実装 | core/typed | medium | enum variant としてはあるが、個別公開型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:13`, `:21`)。 |
| `Terminated` / `ChildFailed` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:81`, `:104` | 部分実装 | core/typed | medium | `Pid` ベースの enum variant で吸収しており、Pekko の dedicated wrapper 型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:15`, `:19`)。 |
| `MessageAdaptionFailure` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:125` | 部分実装 | core/typed | easy | variant はあるが、独立公開型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:17`)。 |

### Receptionist / EventStream　✅ 実装済み 10/12 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receptionist` extension façade (`ref`, `createExtension`, `get`) | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:33`, `:107`, `:108` | 部分実装 | core/typed | easy | fraktor は plain actor と `TypedActorSystem::receptionist_ref/receptionist` で提供 (`modules/actor/src/core/typed/receptionist.rs:42`, `modules/actor/src/core/typed/system.rs:252`)。ExtensionId としては公開していない。 |
| `ServiceKey.Listing` / `ServiceKey.Registered` extractor | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:81`, `:90` | 未対応 | core/typed | easy | `Listing` (`modules/actor/src/core/typed/receptionist/listing.rs:14`) と `Registered` (`modules/actor/src/core/typed/receptionist/registered.rs:16`) はあるが、`ServiceKey` に紐づく extractor helper がない。 |

### Routing / Delivery　✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GroupRouter[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:58` | 未対応 | core/typed | medium | `GroupRouterBuilder` はある (`modules/actor/src/core/typed/dsl/routing/group_router_builder.rs:29`) が、Pekko のような公開 Behavior 型はない。 |
| `PoolRouter[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:131` | 未対応 | core/typed | medium | `PoolRouterBuilder` はある (`modules/actor/src/core/typed/dsl/routing/pool_router_builder.rs:26`) が、公開 Behavior 型がない。 |

### クラシック橋渡し　✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorTags` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:241`, `:255` | 別経路のみ | core/typed | trivial | fraktor は `TypedProps::with_tags/with_tag` (`modules/actor/src/core/typed/props.rs:162`, `:171`) で吸収しており、独立型を持たない。 |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略。

省略理由:
- 型単位カバレッジが 71% に留まる
- `medium` 以上の未実装ギャップが 8 件ある
- 特に typed supervision と signal surface の不足が、内部責務分割より先に parity を阻害している

## 実装優先度

### Phase 1

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `ActorRef.! / RecipientRef.!` | core/typed | 既存 `tell` の薄い alias 追加で済む。 |
| `BehaviorInterceptor.isSame` | core/typed | 既存 trait にデフォルトメソッドを足すだけで閉じる。 |
| `Props.withMailboxFromConfig` | core/typed | `MailboxSelector::from_config` への shorthand を追加するだけで済む。 |
| `Receptionist` extension façade (`ref/get/createExtension`) | core/typed | 既存 `TypedActorSystem::receptionist_ref/receptionist` の薄いラッパーで実装できる。 |
| `ActorTags` façade | core/typed | `TypedProps::with_tags/with_tag` の薄い補助型で済む。 |

### Phase 2

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `SupervisorStrategy.resume/restart/stop` | core/typed | kernel 戦略型を typed ルートに持ち上げる作業。 |
| `RestartSupervisorStrategy` | core/typed | `withLimit` を含む façade 追加が必要。 |
| `BackoffSupervisorStrategy` | core/typed | kernel 実装はあるため、typed parity surface の整備が中心。 |
| `Signal` | core/typed | 既存 `BehaviorSignal` を public wrapper 群へ分解する作業。 |
| `PreRestart` / `PostStop` | core/typed | enum variant を dedicated public type へ切り出す必要がある。 |
| `Terminated` / `ChildFailed` | core/typed | `Pid` だけでなく wrapper 型の追加が必要。 |
| `MessageAdaptionFailure` | core/typed | variant を公開型へ昇格する作業。 |
| `ServiceKey.Listing` / `ServiceKey.Registered` extractor | core/typed | 既存 `Listing` / `Registered` の helper 追加で閉じる。 |
| `GroupRouter[T]` | core/typed | builder 返しではなく public Behavior 型を持たせる必要がある。 |
| `PoolRouter[T]` | core/typed | 同上。 |

### Phase 3

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `ExtensibleBehavior[T]` | core/typed | 既存 `Behavior` / `AbstractBehavior` / interceptors の契約に横断的に触れるため、型設計の再整理が必要。 |

## まとめ

- 全体評価: actor モジュールは receptionist / eventstream / delivery / router builder までかなり前進しているが、**typed の公開契約を Pekko と同じ粒度で見たときに signals と supervision が手薄**。
- parity を低コストで前進できる代表例: `!` alias、`BehaviorInterceptor.isSame`、`withMailboxFromConfig`、`Receptionist` extension façade。
- parity 上の主要ギャップ: `ExtensibleBehavior`、typed `SupervisorStrategy` façade、signal の dedicated public types、`GroupRouter` / `PoolRouter` の公開 Behavior 型。
- 次のボトルネック: まだ API ギャップが支配的であり、現時点では内部モジュール構造より **typed surface の整理** が優先。
