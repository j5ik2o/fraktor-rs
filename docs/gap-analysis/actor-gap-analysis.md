# actor モジュール ギャップ分析

## 前提

- 比較対象:
  - fraktor-rs core/kernel: `modules/actor-core/src/core/kernel/`
  - fraktor-rs core/typed: `modules/actor-core/src/core/typed/`
  - fraktor-rs std: `modules/actor-adaptor-std/src/std/`
  - Pekko classic: `references/pekko/actor/src/main/scala/org/apache/pekko/` (actor, dispatch, event, pattern, routing, serialization, io)
  - Pekko typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`
- カバレッジ数値は `private` / `private[...]` / `internal` を除いた **主要公開契約** を型単位で数えたもの
- classic の Java 継承 DSL (`AbstractActor`, `ReceiveBuilder`, `AbstractActorWithTimers` 等) は JVM / Java モデル依存のため `n/a` 判定
- Java DSL 全般 (`javadsl/`, `japi/`) は `n/a` 判定
- Pekko IO パッケージ (`io/Tcp`, `io/Udp`, `io/Dns` 等) はネットワーク IO モジュールであり、fraktor-rs ではランタイム非依存の actor core に含めず、将来 remote / transport モジュールで扱うため `n/a` 判定
- 分析日: 2026-04-19（初版: 2026-04-15、第2版: 2026-04-16、第3版: 2026-04-17、第4版: 2026-04-17、第5版: 2026-04-17、第6版: 2026-04-17、第7版: 2026-04-18、第8版: 2026-04-19）
- **第8版での重大な是正**: 第7版までは「公開 API カバレッジ 100%」で parity 完了として扱っていたが、これは **型と関数シグネチャの存在** を測ったにすぎず、**内部セマンティクス (実行時の契約)** が Pekko と一致しているかは検証していなかった。第8版では Mailbox / Dispatcher / ActorCell / ChildrenContainer / FaultHandling / DeathWatch / ReceiveTimeout / ActorLifecycle / EventStream / FSM / Stash / SupervisorStrategy の計 34 観点を Pekko 参照実装と行単位で比較し、**high 11 件 / medium 13 件 / low 約 10 件の内部セマンティクス不一致を検出した**。公開 API カウントでのカバレッジ (100%) は維持するが、**"internal parity gap" は Phase A として未解決扱い**とし、本版末尾に「内部セマンティクスギャップ」セクションを追加する。
- 第3版での追加検出: Pekko 側を `actor` / `actor-typed` 両パッケージから全件再抽出し、ergonomics 系 API と classic 補助パターンの未対応項目を新たに洗い出した。
- 第4版での更新: `SmallestMailboxRoutingLogic` の Pekko 互換化を実装完了（2パス探索・`isSuspended`/`isProcessingMessage` 追跡・スコアリング）。部分実装ギャップは 1 件に減少。
- 第5版での更新: 第4版時点の「未対応」3 件が実装済みであることを直接コード確認により再判定。
  - `LoggingFilter` trait: `modules/actor-core/src/core/kernel/event/logging/logging_filter.rs:9` に `pub trait LoggingFilter: Send + Sync { fn should_publish(&self, event: &LogEvent) -> bool; }` が実装済み。`DefaultLoggingFilter` も `default_logging_filter.rs` に実装済み。
  - classic `Pool` / `Group` RouterConfig 基盤: `modules/actor-core/src/core/kernel/routing/{router_config.rs, pool.rs, group.rs}` に `RouterConfig`, `Pool`, `Group` trait が実装済み。動的ルーティー管理用の `RouterCommand` enum (`router_command.rs`) も `GetRoutees` / `AddRoutee` / `RemoveRoutee` / `AdjustPoolSize` variant を持ち Pekko の `RouterManagementMessage` 相当を網羅。
  - `AffinityPool` executor: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/affinity_executor.rs:49` に `pub struct AffinityExecutor` が実装済み。ファイル冒頭に `Pekko equivalent: org.apache.pekko.dispatch.affinity.AffinityPool` と明記されている。
  - `LoggingFilterWithMarker` は、`LogEvent::marker_name` / `marker_properties` フィールドを経由して `LoggingFilter::should_publish(&LogEvent)` から直接参照可能なため、別 trait を切り出す必要がなく `n/a` に再分類。
- 第6版での更新: pekko-porting ワークフローの Batch 1〜3 closing を反映。**Phase 1 の easy 3 件（`ConsistentHashableEnvelope` / `Listeners` 系 / `LoggerOps`）** と **Phase 2 medium の `ConsistentHashingRoutingLogic` 完全化系 3 項目** を判定クロージング済み。
  - `ConsistentHashableEnvelope`（Batch 1）: `modules/actor-core/src/core/kernel/routing/consistent_hashable_envelope.rs` に実装済み。
  - `Listeners` / `Listen` / `Deafen` / `WithListeners`（Batch 1）: `modules/actor-core/src/core/kernel/routing/{listeners.rs, listen.rs, deafen.rs, with_listeners.rs}` に実装済み。
  - `LoggerOps`（Batch 2）: `TypedActorSystemLog` の `trace_fmt` / `debug_fmt` / `info_fmt` / `warn_fmt` / `error_fmt` が lazy formatting 契約（`is_level_enabled` 経由）で Pekko `LoggerOps` 相当のセマンティクスを翻訳済み（`typed_actor_system_log.rs:39-73`）。Rust の `fmt::Arguments<'_>` + `format_args!` によるゼロコストな遅延フォーマットで再表現。
  - `ConsistentHashingRoutingLogic`（Batch 3, 判定クロージング）: rendezvous hashing (HRW) 実装が Pekko 契約 1〜4（stable mapping / minimal disruption / hash key precedence / NoRoutee）を全て満たすことを確認。**partial から完全実装（翻訳）に昇格**。判定根拠と ring 方式との等価性は `docs/plan/pekko-porting-batch-3-consistent-hashing.md` に保存。
  - `ConsistentHash<T>` / `MurmurHash` util / `virtualNodesFactor`（Batch 3）: rendezvous hashing では ring も virtual node も不要なため **非採用（n/a）** として parity 分母から除外。Pekko 側の実装詳細であり、契約意図ではないため移植する意義がない。`ConsistentRoutee` / `AtomicReference` routees キャッシュも同様に非採用。
  - 第6版時点で enumerated gap は **2 件**（core/kernel: `OptimalSizeExploringResizer`、core/typed: typed `OptimalSizeExploringResizer` expose）に縮小。部分実装ギャップは **0 件**。
- 第7版での更新: pekko-porting ワークフロー **Batch 4 closing** を反映。**Phase 3 hard の `OptimalSizeExploringResizer`（classic + typed expose）** を判定クロージング済み。
  - Pekko 側 `DefaultOptimalSizeExploringResizer`（`references/pekko/actor/src/main/scala/org/apache/pekko/routing/OptimalSizeExploringResizer.scala:L59`）の 3 アクション（downsize / explore / optimize）と 10 チューニングパラメータを、**typed DSL 層**（`modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs`）に 1 つの公開型として翻訳実装。
  - Pekko の `ThreadLocalRandom` 依存を `Clock: Send + Sync` trait + シード可能 LCG（Numerical Recipes MMIX 定数、`optimal_size_exploring_resizer/lcg.rs`）に置換し、決定的な explore / optimize 分岐を実現。
  - Pekko 側の `Resizer.resize(currentRoutees: IndexedSeq[ActorRef])` は fraktor-rs で `Resizer::resize(mailbox_sizes: &[usize])` として既に運用されていたが、`OptimalSizeExploringResizer` は各メッセージでのメトリクス観測を要するため **`Resizer::report_message_count(&[usize], u64)` を default no-op で新規追加**（契約破壊なし。`DefaultResizer` は未実装のまま既存動作を継続）。
  - `PoolRouter` 側は `observe_routee_mailbox_sizes` ヘルパで `ActorRef::system_state()` 経由の `Mailbox::user_len()` スナップショットを取得し、毎メッセージで `report_message_count` を呼び、`is_time_for_resize` 真のときに同じスナップショットを `resize` に渡す形へ配線変更（Pekko `ResizablePoolCell.sendMessage` と同等順序）。
  - 内部状態（`performance_log: BTreeMap<usize, Duration>` / `under_utilization_streak` / `message_count` / `total_queue_length` / `check_time` / `rng`）は `SpinSyncMutex` 1 本に集約し、`Resizer` trait の `&self` 契約を保つ。**第2層の `*Shared` ラッパーは作らず、`DefaultResizer` / `CircuitBreaker` と同じ「1 型 + 内部ロック」パターンを踏襲**（`immutability-policy.md` の軽微逸脱は `DefaultResizer` 前例踏襲として明示的に許容、判定根拠: `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md`）。
  - Pekko の `var checkTime = 0L` センチネルは、抽象 `Clock::Instant` に対して意味のある 0 値を定義できないため `has_recorded: bool` + `check_time: I` のペアに置換。
  - no_std 配下で `f64::ceil` / `f64::floor` が使えないため、`libm_ceil` / `libm_floor` を実装内のフリー関数として用意（`libm` クレート依存を回避）。
  - 非採用: `akka.routing.MetricsBasedResizer`（Pekko 実装では `DefaultOptimalSizeExploringResizer` にインライン化されており独立公開型ではないため parity 対象外）。`ThreadLocalRandom` 共有（決定性失う）。Scala の `var` による state mutation（Rust 借用システムで置換済）。`weightedAverage` の separate util 化（5 行の内部 helper のため inline 保持）。
  - 第7版時点で enumerated gap は **0 件**（全カテゴリ parity 完全達成）。部分実装ギャップも **0 件**。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（parity 対象） | 101 |
| fraktor-rs 対応実装数 | 101（すべて**型レベルでは**存在） |
| カバレッジ（型単位） | 101/101 = 100% |
| 公開 API ギャップ数 | 0（未対応 0、部分実装 0） |
| **内部セマンティクスギャップ数 (第8版追加)** | **24+（high 11 / medium 13 / low 約 10）** |
| n/a 除外数 | 約 62（Java DSL, IO, japi, internal, JVM 固有、`LoggingFilterWithMarker`、`ConsistentHash<T>` / `MurmurHash` util グループ、`virtualNodesFactor`、`AtomicReference` routees cache、`ConsistentRoutee` wrapper） |

enumerated gaps (公開 API): **なし**（第7版 Batch 4 closing で `OptimalSizeExploringResizer` / typed expose の 2 件を同時 closing）。

enumerated gaps (内部セマンティクス): **24+ 件** — 詳細は本版末尾の「内部セマンティクスギャップ」セクションを参照。

### カバレッジの読み替え

第7版までの「101/101 = 100%」は **公開型と公開メソッドシグネチャが揃っている** ことを意味するが、**Pekko と同じ契約 (semantics) で動く** ことは保証していなかった。第8版で検出された内部セマンティクス不一致を加味すると、**実効カバレッジは約 60-70%** と推定される (101 型のうち 24 型程度で内部契約が Pekko から逸脱)。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 39 | 39 | 39/39 = 100% |
| core / typed ラッパー | 56 | 56 | 56/56 = 100% |
| std / アダプタ | 6 | 6 | 6/6 = 100% |
| 合計 | 101 | 101 | 101/101 = 100% |

`std` は Pekko の JVM 依存ランタイム補助（ロギング、スレッド実行器、協調停止、時計/回路遮断器相当）に対応づけている。

core / untyped kernel の母数が 40 → 39 に減ったのは、第6版で `ConsistentHash<T>` / `MurmurHash` util グループを非採用（n/a）に再分類したため（rendezvous hashing 実装では ring も MurmurHash も不要。詳細は `docs/plan/pekko-porting-batch-3-consistent-hashing.md`）。

第7版で core / untyped kernel と core / typed ラッパーが 100% に到達したのは、`OptimalSizeExploringResizer` を **typed DSL 層に 1 つの公開型として翻訳実装**（classic の Pekko 側構造をそのまま移植するのではなく、`PoolRouter::with_resizer` から即座に使えるレイヤーに集約）し、同時に core/kernel 側でも Pekko の契約意図を満たす実装として parity カウント対象に含めたため。実装ファイルは `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs`（+ `lcg.rs` / `state.rs` / `resize_record.rs` / `under_utilization_streak.rs` の 4 サブモジュール）。判定根拠: `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md`。

## カテゴリ別ギャップ

### classic actor core ✅ 実装済み 16/16 (100%)

ギャップなし。`PoisonPill`（`poison_pill.rs`）と `Kill`（`kill.rs`）が独立した公開 newtype として実装済み。いずれも `From<PoisonPill> for SystemMessage` / `From<Kill> for SystemMessage` 変換を提供。

実装済み型: `Actor` trait, `ActorCell`, `ActorContext`, `ActorPath`, `RootActorPath`, `ChildActorPath`, `ActorRef`, `DeadLetter`, `DeadLetterEntry`, `DeadLetterReason`, `DeadLetterShared`, `ActorIdentity`, `Identify`, `ActorSelection`, `Props`, `Address`, `ReceiveTimeout`, `PoisonPill`, `Kill`, `on_terminated` (Actor trait lifecycle hook)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AbstractActor` / `ReceiveBuilder` | `AbstractActor.scala` | n/a | - | n/a | Java 継承 DSL。Rust では `Actor` trait と関数/クロージャで代替 |
| `AbstractActorWithTimers` 等 Java mixin 群 | `AbstractActor.scala`, `Timers.scala` | n/a | - | n/a | Java mixin API。意味的には `ActorContext::timers()` / `ClassicTimerScheduler` で代替 |

### supervision / fault handling ✅ 実装済み 8/8 (100%)

ギャップなし。`SupervisorStrategy`, `SupervisorStrategyKind` (OneForOne / AllForOne), `SupervisorDirective` (Resume/Restart/Stop/Escalate), `SupervisorStrategyConfig`, `RestartStatistics`, `BackoffSupervisorStrategy`, `BackoffOnFailureOptions`, `BackoffOnStopOptions`, `BackoffSupervisor` は主要契約をカバー。

### typed core surface ✅ 実装済み 36/36 (100%)

ギャップなし。前回分析時に未対応・部分実装としていた以下の4型がすべて独立した公開型として実装済みであることを確認:

- `ExtensibleBehavior`（`extensible_behavior.rs`）: `receive` と `receive_signal` メソッドを持つ公開 trait。`Behaviors::from_extensible` で `Behavior` に変換可能
- `Terminated`（`message_and_signals/terminated.rs`）: `TypedActorRef<Infallible>` を保持する独立 struct。`Signal` trait と `From<Terminated> for BehaviorSignal` を実装
- `ChildFailed`（`message_and_signals/child_failed.rs`）: `Terminated` + `ActorError` を保持する独立 struct。Pekko と同様に `Terminated` のサブタイプ関係を合成で表現
- `MessageAdaptionFailure`（`message_and_signals/message_adaption_failure.rs`）: `AdapterError` を保持する独立 struct。`Signal` trait と `From<MessageAdaptionFailure> for BehaviorSignal` を実装

実装済み型: `Behavior`, `Receive`, `Behaviors` (setup/receive/receiveMessage/withTimers/withStash/logMessages/withMdc/intercept/transformMessages/monitor/stopped), `TypedActorContext`, `TypedActorRef`, `TypedActorSystem`, `ActorRefResolver`, `AbstractBehavior` trait, `ExtensibleBehavior` trait, `BehaviorInterceptor`, `BehaviorSignalInterceptor`, `BehaviorSignal`, `Terminated`, `ChildFailed`, `MessageAdaptionFailure`, `PreRestart`, `PostStop`, `DeathPactError`, `Signal` trait, `LogOptions`, `DispatcherSelector`, `MailboxSelector`, `TypedProps`, `ActorTags`, `SpawnProtocol`, `RecipientRef`, `MessageAdapterRegistry`, `AdapterPayload`, `TypedAskFuture`, `TypedAskResponse`, `StatusReply`, `FsmBuilder`, `BackoffSupervisorStrategy`, `RestartSupervisorStrategy`, `SupervisorStrategy` (typed)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `BehaviorBuilder` (Java DSL) | `javadsl/BehaviorBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `ReceiveBuilder` (Java DSL) | `javadsl/ReceiveBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `AbstractMatchingBehavior` (Java DSL) | `javadsl/AbstractMatchingBehavior.scala` | n/a | - | n/a | Java DSL 専用 |

### dispatch / mailbox ✅ 実装済み 13/13 (100%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| ~~`NonBlockingBoundedMailbox`~~ | ~~`Mailbox.scala:L688`~~ | ~~未対応~~ | ~~core/kernel~~ | ~~easy~~ | **候補から除外**: `BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest)`（`bounded_message_queue.rs:26`）が意味的に等価（enqueue は非ブロッキングかつ overflow 時に即破棄）。Pekko の独立型は Lock-free MPSC という実装選択のための別名であり、fraktor-rs では overflow strategy の選択肢として統合済み。名前だけの parity のため追加実装は不要。 |

実装済み型: `Mailbox`, `MessageQueue` trait, `MailboxType` trait, `Envelope`, `UnboundedMessageQueue`, `BoundedMessageQueue`, `UnboundedDequeMessageQueue`, `UnboundedPriorityMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedStablePriorityMessageQueue`, `BoundedStablePriorityMessageQueue`, `UnboundedControlAwareMessageQueue`, `UnboundedControlAwareMailboxType`, `MessagePriorityGenerator` trait, `MailboxCapacity`, `MailboxOverflowStrategy`, `MailboxPolicy`, `Mailboxes`, `Dispatchers`, `DefaultDispatcher`, `PinnedDispatcher`, `BalancingDispatcher`, `Executor` trait, `InlineExecutor`, `MessageDispatcher` trait, `DispatcherSettings`, `SharedMessageQueue`

### event / logging ✅ 実装済み 10/10 (100%)

第6版での更新: `LoggerOps` を Batch 2 で翻訳済み。Pekko の `LoggerOps.trace/debug/info/warn/error(template, args...)` の **lazy formatting** 契約を、Rust の `fmt::Arguments<'_>` + `is_level_enabled` ショートサーキットで再表現（`typed_actor_system_log.rs` の `trace_fmt` / `debug_fmt` / `info_fmt` / `warn_fmt` / `error_fmt`）。Pekko では Scala の by-name 引数 + implicit で `LoggerOps` を提供しているが、Rust では `format_args!` マクロが同等のゼロコスト遅延評価を提供するため、専用 trait を追加せず `TypedActorSystemLog` に inherent method として統合。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LoggingFilterWithMarker` | `Logging.scala:L1604` | n/a | - | n/a | Pekko では `LoggingFilter` を拡張し marker を引数で受ける専用 trait。fraktor-rs では `LogEvent` が `marker_name` / `marker_properties` を保持しているため、既存の `LoggingFilter::should_publish(&LogEvent)` から直接参照可能。別 trait を切り出す意味がないため対象外 |

実装済み型: `EventStream`, `EventStreamSubscriber` trait, `EventStreamSubscription`, `LogEvent`, `LogLevel`, `LoggingAdapter`, `BusLogging`, `NoLogging`, `ActorLogging`, `DiagnosticActorLogging`, `ActorLogMarker`, `LoggingReceive`, `LoggingFilter` trait, `DefaultLoggingFilter`, `LoggerSubscriber` (core), `TracingLoggerSubscriber` / `DeadLetterLogSubscriber` (std), `TypedActorSystemLog::{trace,debug,info,warn,error}_fmt` (`LoggerOps` 翻訳)

備考: Pekko の `EventBus` trait（EventStream とは別の汎用イベントバス抽象）は未実装だが、fraktor では `EventStreamSubscriber` trait が同等の役割を果たしており、実質的な機能欠落はない。独立した汎用 `EventBus` trait の必要性は低い。`Logging.Error/Warning/Info/Debug` 独立 case class は fraktor の `LogEvent` 列挙型で機能的にカバー済みのため parity 対象外。

### pattern ✅ 実装済み 5/5 (100%)

ギャップなし。前回分析時に未対応としていた `CircuitBreakersRegistry` が `modules/actor-adaptor-std/src/std/pattern/circuit_breakers_registry.rs` に実装済みであることを確認。`Extension` trait を実装し、`from_actor_system` / `get` / `with_named_config` 等のメソッドで名前ベースの CB インスタンス管理を提供。

実装済み型: `CircuitBreaker`, `CircuitBreakerShared`, `CircuitBreakerState`, `CircuitBreakerOpenError`, `CircuitBreakerCallError`, `Clock` trait, `CircuitBreakersRegistry`, `ask_with_timeout`, `graceful_stop`, `graceful_stop_with_message`, `retry`, `pipe_to` / `pipe_to_self` (ActorContext メソッド)

### classic routing ✅ 実装済み 15/15 (100%)

第7版での更新（Batch 4 closing）:
- **Batch 4**: `OptimalSizeExploringResizer` を typed DSL 層に翻訳実装し、**classic routing の parity カウント対象にも含める**（Pekko 側は classic 下に配置されるが、fraktor-rs では typed 側にのみ実装。`Resizer` trait 自体は classic / typed 共通のため `PoolRouter` から利用可能）。3 アクション（downsize / explore / optimize）・10 チューニングパラメータ・性能記録 BTreeMap・LCG ベースの決定的 RNG を統合。詳細: `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md`。

第6版での更新（Batch 1 / Batch 3 closing）:
- **Batch 1**: `ConsistentHashableEnvelope`（`consistent_hashable_envelope.rs`）、`Listeners` / `Listen` / `Deafen` / `WithListeners`（`listeners.rs`, `listen.rs`, `deafen.rs`, `with_listeners.rs`）を実装済み。
- **Batch 3**: `ConsistentHashingRoutingLogic` の実装が Pekko 契約 1〜4（stable mapping / minimal disruption / hash key precedence / NoRoutee）を rendezvous hashing (HRW) + FNV mix で満たすことを判定クロージング。partial 扱いから **完全実装（翻訳）** に昇格。`ConsistentHash<T>` / `MurmurHash` util グループは rendezvous では ring も MurmurHash も不要なため **非採用（n/a）** に再分類。`virtualNodesFactor` / `AtomicReference` routees cache / `ConsistentRoutee` も同様に非採用。詳細は `docs/plan/pekko-porting-batch-3-consistent-hashing.md`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| ~~`OptimalSizeExploringResizer`~~ | ~~`OptimalSizeExploringResizer.scala:L59`~~ | ~~未対応~~ | ~~core/kernel~~ | ~~hard~~ | ✅ Batch 4 で翻訳実装クロージング。typed DSL 層の `OptimalSizeExploringResizer` として実装（`modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs`）。詳細: `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md` |
| `ConsistentHash<T>` / `MurmurHash` | `ConsistentHash.scala`, `MurmurHash.scala` | n/a | - | n/a | rendezvous hashing (HRW) の採用により ring 自体が不要。Pekko 内部実装詳細であり契約意図ではない。判定根拠: `docs/plan/pekko-porting-batch-3-consistent-hashing.md` |

実装済み型 (kernel): `RoutingLogic` trait, `RouterConfig` trait, `Pool` trait, `Group` trait, `Router`, `Routee`, `Broadcast`, `RandomRoutingLogic`, `RoundRobinRoutingLogic`, `ConsistentHashingRoutingLogic`（Pekko 契約 1〜4 を rendezvous hashing で翻訳）, `ConsistentHashable` trait, `ConsistentHashableEnvelope`, `SmallestMailboxRoutingLogic`（Pekko 互換完全実装: 2パス探索・`isSuspended`/`isProcessingMessage` 追跡・スコアリング）, `RouterCommand` (GetRoutees/AddRoutee/RemoveRoutee/AdjustPoolSize), `RouterResponse`, `Listeners` struct, `Listen`, `Deafen`, `WithListeners`

備考: classic の `Pool` / `Group` / `RouterConfig` trait は kernel に `router_config.rs` / `pool.rs` / `group.rs` として揃っており、typed 側の `PoolRouter` / `GroupRouter` はこれを直接 impl する形で構築されている。`RouterCommand` の variant も Pekko `RouterManagementMessage` 相当を網羅している。

### typed routing ✅ 実装済み 7/7 (100%)

第7版での更新（Batch 4 closing）: `OptimalSizeExploringResizer` を typed DSL 層に実装し、`PoolRouter::with_resizer` から `Arc::new(OptimalSizeExploringResizer::new(...))` を渡す形で即座に利用可能になった。

実装済み型: `Routers`, `PoolRouter`, `GroupRouter`, `BalancingPoolRouterBuilder`, `ScatterGatherFirstCompletedRouterBuilder`, `TailChoppingRouterBuilder`, `DefaultResizer`, `OptimalSizeExploringResizer`, `Resizer` trait（`resize(&[usize]) -> i32` + `report_message_count(&[usize], u64)` default no-op）。ConsistentHash / SmallestMailbox は `PoolRouter` / `GroupRouter` のメソッドとして利用可能。

### discovery / receptionist ✅ 実装済み 9/9 (100%)

ギャップなし。`Receptionist`, `ServiceKey`, `Register`, `Deregister`, `Subscribe`, `Find`, `Listing`, `Registered`, `Deregistered` は主要契約をカバー。

### scheduling / timers ✅ 実装済み 8/8 (100%)

ギャップなし。classic `Scheduler` / `ClassicTimerScheduler` / `Cancellable` (`= SchedulerHandle`)、typed `Scheduler` / `TimerScheduler` / `TimerKey` は実装済み。

### ref / resolution ✅ 実装済み 6/6 (100%)

ギャップなし。`ActorRef`, `ActorSelection`, `ActorPath`, `ActorRefResolver`, `narrow`, `unsafe_upcast`, `to/from serialization format` まで揃っている。

### delivery / pubsub ✅ 実装済み 8/8 (100%)

ギャップなし。`ProducerController`, `ConsumerController`, `DurableProducerQueue`, `Topic`, `TopicStats`, `WorkPullingProducerController`, `SequencedMessage`, `WorkerStats` まで揃っている。

### serialization ✅ 実装済み 8/8 (100%)

ギャップなし。`Serializer` trait, `SerializerWithStringManifest`, `ByteBufferSerializer`, `AsyncSerializer`, `SerializationExtension`, `SerializationRegistry`, `SerializationSetup`, `SerializedMessage`, `SerializerId`, `SerializationDelegator`, builtin serializers (Bool/ByteString/Bytes/I32/Null/String) まで揃っている。Pekko の `JavaSerializer` / `DisabledJavaSerializer` は JVM 固有のため n/a。

### extension ✅ 実装済み 4/4 (100%)

ギャップなし。`Extension` trait, `ExtensionId` trait, `ExtensionInstaller` trait, `ExtensionInstallers` は実装済み。typed 側も `ExtensionSetup`, `AbstractExtensionSetup` を提供。

### coordinated shutdown ✅ 実装済み 5/5 (100%)

ギャップなし。`CoordinatedShutdown`, `CoordinatedShutdownPhase`, `CoordinatedShutdownReason`, `CoordinatedShutdownInstaller`, `CoordinatedShutdownId` は実装済み。

### std adaptor ✅ 実装済み 6/6 (100%)

ギャップなし。第5版の再調査で `AffinityExecutor` (`modules/actor-adaptor-std/src/std/dispatch/dispatcher/affinity_executor.rs:49`) が Pekko `AffinityPool` 相当として実装済みであることを確認。ファイル冒頭に `Pekko equivalent: org.apache.pekko.dispatch.affinity.AffinityPool` と明記されており、`AffinityExecutorFactory` (`affinity_executor_factory.rs`) で生成される。

`VirtualThreadExecutorConfigurator` は JVM 固有（Java 21+ 仮想スレッド）であり、Rust では tokio / smol が同等のスケジューリングを提供するため `n/a` に分類。

実装済み型 (std): `TokioExecutor`, `TokioExecutorFactory`, `PinnedExecutor`, `PinnedExecutorFactory`, `ThreadedExecutor`, `AffinityExecutor`, `AffinityExecutorFactory`, `StdClock`, `StdBlocker`, `TracingLoggerSubscriber`, `DeadLetterLogSubscriber`, `StdTickDriver`, `TokioTickDriver`

## 内部モジュール構造ギャップ

第7版で API ギャップが **100%** 埋まったため、残る改善余地は **内部モジュール構造の整理** のみ。以下は parity カウント対象外だが、今後の保守性のために継続的に改善する。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| receptionist の facade / protocol / runtime 実装がまだ粗く同居 | `actor-typed/receptionist/Receptionist.scala`, `actor-typed/internal/receptionist/ReceptionistMessages.scala` | `core/typed/receptionist.rs` が facade + behavior を保持し、protocol 型だけ `receptionist/` 配下に分割 | `core/typed/receptionist/` に behavior 実装も寄せ、公開 facade と内部実装の境界を明確化 | medium | high | 今後 serializer / cluster receptionist 拡張を入れると 1 ファイル集中が重くなる |
| typed delivery に `internal` 層がなく、公開型と制御ロジックが同じ階層に並ぶ | `actor-typed/delivery/*`, `actor-typed/delivery/internal/ProducerControllerImpl.scala` | `core/typed/delivery/` 直下に command / settings / behavior / state が並列 | `delivery/internal/` を新設し、controller 実装詳細と公開 DTO を分離 | medium | medium | 現時点で API は揃っているが、再送・永続キュー拡張時に責務が散りやすい |
| classic kernel の public surface が広く、内部補助型まで `pub` に露出しやすい | Pekko classic は package-private / internal API が多い | `core/kernel/**` に利用者向けでない `pub` 型が広く存在 | `pub(crate)` へ寄せられるものを継続的に縮小し、入口 facade からの再公開を基準に露出制御 | medium | medium | fraktor は `pub` 露出が多く、型数だけで見ると Pekko を上回る |

備考: 第5版まで記載していた「classic routing の kernel 層 `ConsistentHashingRoutingLogic` が簡略実装」は、第6版（Batch 3 判定クロージング）で rendezvous hashing (HRW) 実装が Pekko 契約 1〜4 を満たすことを確認し、構造ギャップ表から削除した。判定根拠は `docs/plan/pekko-porting-batch-3-consistent-hashing.md`。

## 実装優先度

### Phase 1（trivial / easy）— ✅ 全項目 closing 済み

第6版時点で全 3 項目を closing 済み（Batch 1 / Batch 2）:

| 項目 | 実装先層 | closing バッチ | 成果物 |
|------|----------|---------------|--------|
| `ConsistentHashableEnvelope` | core/kernel | Batch 1 | `consistent_hashable_envelope.rs` |
| `Listeners` trait / `Listen` / `Deafen` / `WithListeners` | core/kernel | Batch 1 | `listeners.rs`, `listen.rs`, `deafen.rs`, `with_listeners.rs` |
| `LoggerOps` 相当の lazy formatting log helpers | core/typed | Batch 2 | `TypedActorSystemLog::{trace,debug,info,warn,error}_fmt`（`typed_actor_system_log.rs:39-73`） |

### Phase 2（medium）

ConsistentHashingRoutingLogic 系は Batch 3 で判定クロージング済み。残項目は構造整理のみ:

| 項目 | 実装先層 | 状態 | 理由 |
|------|----------|------|------|
| ~~`ConsistentHashingRoutingLogic` 完全化~~ | ~~core/kernel~~ | ✅ Batch 3 で翻訳判定クロージング | rendezvous hashing (HRW) が Pekko 契約 1〜4 を満たすことを確認。partial → 完全実装（翻訳）に昇格。詳細: `docs/plan/pekko-porting-batch-3-consistent-hashing.md` |
| ~~`ConsistentHash<T>` / `MurmurHash` util 公開~~ | ~~core/kernel (util)~~ | ✅ Batch 3 で非採用（n/a） | rendezvous では ring 自体が不要。Pekko 内部実装詳細であり契約意図ではない |
| receptionist 実装の `receptionist/` 配下への再配置 | core/typed | 未着手 | API を壊さず責務を整理できるが、ファイル分割は複数箇所に波及する |
| delivery の `internal` 分離 | core/typed | 未着手 | 既存 controller 群の責務整理が必要 |

### Phase 3（hard）— ✅ 全項目 closing 済み

第7版時点で全 1 項目を closing 済み（Batch 4）:

| 項目 | 実装先層 | closing バッチ | 成果物 |
|------|----------|---------------|--------|
| ~~`OptimalSizeExploringResizer` (classic + typed expose)~~ | ~~core/kernel + core/typed~~ | ✅ Batch 4 | `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs` + `lcg.rs` / `state.rs` / `resize_record.rs` / `under_utilization_streak.rs`、`Resizer::report_message_count` default no-op を trait に追加、`PoolRouter::observe_routee_mailbox_sizes` で `Mailbox::user_len()` スナップショット配線。判定根拠: `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md` |

### 対象外（n/a）

| 項目 | 理由 |
|------|------|
| `AbstractActor` / `ReceiveBuilder` 等 Java 継承 DSL | JVM / Java 継承モデル依存。Rust の `Actor` trait + closure で代替 |
| `AbstractActorWithTimers` 等 Java mixin 群 | JVM / Java mixin 依存。`ClassicTimerScheduler` / typed `TimerScheduler` でカバー |
| `BehaviorBuilder` / `ReceiveBuilder` (Java DSL) | Java DSL 専用 |
| `AbstractMatchingBehavior` (Java DSL) | Java DSL 専用 |
| IO パッケージ (`Tcp`, `Udp`, `Dns` 等) | ネットワーク IO は remote / transport モジュールで扱う。actor core の parity 対象外 |
| `JavaSerializer` / `DisabledJavaSerializer` | JVM Java シリアライゼーション固有 |
| `japi/` パッケージ全体 | Java API interop 層 |
| `VirtualThreadExecutorConfigurator` / `VirtualizedExecutorService` | JVM 固有（Java 21 仮想スレッド）。Rust では tokio が同等 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | JVM クラスローダー/リフレクション固有 |
| `IndirectActorProducer` / `TypedCreatorFunctionConsumer` | JVM クラスベースの Actor 生成。Rust では `Props` + closure で代替 |
| `ProviderSelection` | JVM の ActorSystem プロバイダ選択機構。fraktor-rs では不要 |
| `LoggingFilterWithMarker` | `LogEvent` が marker フィールドを保持しているため既存 `LoggingFilter::should_publish(&LogEvent)` で代替可能。別 trait は不要 |
| `ConsistentHash<T>` (sorted hash ring 構造) | rendezvous hashing (HRW) を採用したため ring 自体が不要。Pekko 内部実装詳細であり、契約意図（stable mapping / minimal disruption）は rendezvous で等価に満たされる。判定根拠: `docs/plan/pekko-porting-batch-3-consistent-hashing.md` |
| `MurmurHash` util | 上記 `ConsistentHash<T>` の構成要素。rendezvous hashing では 64bit FNV mix (`mix_hash`) で `(key, routee_identity)` を混ぜる方式に置き換え済み。Murmur 相当の独立 util を公開する必要がない |
| `virtualNodesFactor` パラメータ | sorted ring 上の virtual node 密度を調整するパラメータ。rendezvous hashing は構造的に均等分布のため no-op knob となり、公開すると利用者を誤導する |
| `AtomicReference` routees cache | Pekko は `(routees, ring)` 再計算を避けるために `AtomicReference` で直前結果をキャッシュするが、rendezvous は `O(n)` per call でキャッシュ対象の構造を持たない。かつ `immutability-policy.md` により内部可変性は禁止。両面から採用不可 |
| `ConsistentRoutee` wrapper | Pekko はクラスタ環境で routee に `selfAddress` を紐付けるため追加ラッパーを用意している。fraktor-rs の `Routee::ActorRef` は `Pid (value + generation)` を既に一意識別子として保持しており、同レイヤーでのラップが不要 |

## 内部セマンティクスギャップ（第8版追加）

公開 API 数としては 101/101 だが、**Pekko と同じ実行時契約で動いているか** を行単位で比較した結果、以下の 24 件（high 11 / medium 13）の内部セマンティクス不一致を検出した。これらは「型の存在」では捕捉できず、Pekko 参照実装を直接読んで挙動を突合しないと出てこないため、第7版までの公開 API カウント手法では見逃されていた。

### Mailbox 系（3 high / 3 medium）

| ID | 観点 | Pekko (ファイル:行) | fraktor-rs (ファイル:行) | 不一致の内容 | 深刻度 |
|----|------|--------------------|------------------------|-------------|--------|
| MB-H1 | Suspend 時の enqueue 契約 | `Mailbox.scala:99` — enqueue は常に受理、suspend は dequeue/processing 側で制御 | `mailbox/base.rs:418-421` — `enqueue_envelope` が `is_suspended()` 時に `SendError::suspended` を返して **メッセージを破棄** | Pekko は「suspend 中に到着したメッセージを再開後に処理する」契約。fraktor-rs は suspend 中のメッセージを失う | **high** |
| MB-H2 | Cleanup 時の system queue → DeadLetters 転送 | `Mailbox.scala:288-330,338-352` — `cleanUp()` で system / user 両キューを drain して DeadLetters に送る | `mailbox/base.rs:621-637` — `finalize_cleanup` は `user` キューのみドレイン、system キューは触らない | shutdown 時に system message が握りつぶされ、watcher への Terminated 通知等が消失し得る | **high** |
| MB-H3 | Bounded overflow 時の DeadLetters 通知 | `Mailbox.scala:428-432` — `BoundedNodeMessageQueue.enqueue` が overflow 時に `deadLetters.tell(DeadLetter(...))` を送る | `bounded_message_queue.rs:34-40` — `DropNewest`/`DropOldest`/`Grow` でサイレントドロップ、DL 通知なし | DeadLetter 追跡ができず、メッセージロスが観測不能 | **high** |
| MB-M1 | Throughput deadline | `Mailbox.scala:264-276` — `System.nanoTime` ベースで deadline 超過を検知して即 reschedule | `mailbox/base.rs:256` — `_throughput_deadline: Option<Duration>` 引数を受け取るが `_` プレフィックスで未使用 (`// Deadline support is added in a follow-up change.` と明記) | 特定 actor が throughput を使い切るまで dispatcher を占有する懸念 | medium |
| MB-M2 | BoundedDequeBasedMailbox / BoundedControlAwareMailbox | `Mailbox.scala:844,931` — 両型あり | 両型とも **未実装** | Stash + bounded の組み合わせ、Control-aware + bounded の組み合わせが選択不可 | medium |
| MB-M3 | Blocking push-timeout 戦略 | `Mailbox.scala:556-566` — `pushTimeOut` 経由で `queue.offer(handle, timeout)` により送信側を block | 未実装（非同期 Rust で blocking は設計外） | Rust 設計上は妥当だが、Pekko ユーザーの backpressure 期待値とは異なる | medium |

### Dispatcher / ActorCell 系（5 high / 5 medium）

| ID | 観点 | Pekko (ファイル:行) | fraktor-rs (ファイル:行) | 不一致の内容 | 深刻度 |
|----|------|--------------------|------------------------|-------------|--------|
| AC-H1 | user msg 1 件ごとの system msg flush | `Mailbox.scala:274` — `processMailbox()` の再帰ループ内で user msg 1 件処理後に必ず `processAllSystemMessages()` を呼ぶ | `mailbox/base.rs:283-316` — system/user を throughput limit 内で混在処理、user 1 件ごとの system flush なし | Suspend / Failed / Terminated 等の system message が user message の後ろで遅延処理される。「suspend 通知後も throughput 分だけ user が流れる」現象 | **high** |
| AC-H2 | ChildrenContainer の 4 状態 state machine | `dungeon/ChildrenContainer.scala` — `EmptyChildrenContainer` / `NormalChildrenContainer` / `TerminatingChildrenContainer` / `TerminatedChildrenContainer` + `SuspendReason(Recreation/Creation/Termination)` | `actor_cell_state.rs:15` — `children: Vec<Pid>` のフラットリストのみ | restart/terminate の「子の終了完了を待つ」非同期フェーズが実装不可 | **high** |
| AC-H3 | faultSuspend / faultResume の子への再帰伝播 | `dungeon/FaultHandling.scala:124-153` — `suspendNonRecursive` + `suspendChildren` の pair、`resumeNonRecursive` + `resumeChildren` で子全員に再帰 | `actor_cell.rs:1118-1125` — 自 mailbox の suspend/resume のみ、子への再帰なし | 親 actor が suspend されても子は稼働し続ける。Pekko の「障害隔離中は子も停止」契約違反 | **high** |
| AC-H4 | restart 中の子停止完了待ち | `FaultHandling.scala:100-118` — `faultRecreate` → `aroundPreRestart` → `setChildrenTerminationReason(Recreation)` → (全子 Terminated 受信) → `finishRecreate` → `aroundPostRestart` → mailbox.resume | `actor_cell.rs:864` — `handle_recreate` が `pre_restart → drop_actor → recreate_actor → pre_start` を直列実行、子停止の完了待ちなし | restart 完了時に子がまだ生きており、古い子インスタンスがメッセージを処理し続ける可能性 | **high** |
| AC-H5 | terminatedQueued による遅延 Terminated delivery | `dungeon/DeathWatch.scala:32,111` — `terminatedQueued: Map[ActorRef, Option[Any]]` に記録して user キュー経由で delivery、`unwatch` で `terminatedQueued -= a` で取消 | `actor_cell.rs:819-830` — `handle_terminated` がシステムメッセージ処理内で直接 `on_terminated` を呼ぶ | `unwatch` 後に遅延到着した `Terminated` を握り潰せず、誤配信が起きる | **high** |
| AC-M1 | PinnedDispatcher の actor 登録排他チェック | `PinnedDispatcher.scala:48-53` — `if ((actor ne null) && actorCell != actor) throw` で複数 actor の共有を禁止 | `pinned_executor.rs` — 排他ガードなし、複数 actor が同じ dispatcher を resolve するとスレッドが共有される | pinned 契約「1 actor 1 thread」が破れる可能性 | medium |
| AC-M2 | Dispatcher の config 動的ロード + alias 解決 | `Dispatchers.scala:159-180` — HOCON `type` キーから動的ロード、`Dispatchers.scala:176` で id chain の再帰解決 | `dispatchers.rs:118` — `register()` 経由の静的登録のみ、alias 連鎖解決なし | `dispatcher.foo = "akka.actor.default-dispatcher"` のような設定が動かない | medium |
| AC-M3 | FailedFatally / isFailed ガード | `FaultHandling.scala:73-74` — `isFailed` で重複 fail 抑制、`isFailedFatally` で以降の復旧を完全拒否 | なし | 失敗中に同一 actor へ複数 fail が来た時の競合や、fatal 障害の永続マーキングが欠落 | medium |
| AC-M4 | watchWith 重複チェック + address terminated 購読 | `DeathWatch.scala:40,127-132` — `maintainAddressTerminatedSubscription` でノード障害通知を購読、`checkWatchingSame` で重複 watchWith を `IllegalStateException` で拒否 | なし | クラスタ環境でのノード障害通知が受信できず、`watchWith(a, M1)` と `watchWith(a, M2)` の二重登録が静かに発生 | medium |
| AC-M5 | NotInfluenceReceiveTimeout マーカー | `ReceiveTimeout.scala:72` — `NotInfluenceReceiveTimeout` 実装メッセージは timeout を reset しない (`ReceiveTimeout` 自身も含む) | `actor_cell.rs:1087` — user msg 成功後は常に `reschedule_receive_timeout()` | `ReceiveTimeout` 自身が届いた時に timeout が自己リセットされ、二重発火の可能性 | medium |

### EventStream / Scheduler / FSM / Stash / Supervision 系（3 high / 5 medium）

| ID | 観点 | Pekko (ファイル:行) | fraktor-rs (ファイル:行) | 不一致の内容 | 深刻度 |
|----|------|--------------------|------------------------|-------------|--------|
| ES-H1 | Classifier (サブクラス関係) | `EventBus.scala` — `SubchannelClassification` + `isAssignableFrom` で型ヒエラルキー fan-out (`Animal` 購読で `Dog` も受信) | `event_stream_shared.rs` — `EventStreamEvent` closed enum、subscriber は全 variant を受信、型ヒエラルキーなし | サブクラス指定購読が不可能。subscriber 側で match が必要 | **high** |
| SP-H1 | Decider の粒度 (JVM Error → Escalate) | `SupervisorStrategy.scala:defaultDecider` — `Error` (JVM fatal) は必ず `Escalate` | `base.rs:default_decider` — `ActorError::Fatal => Stop` (hard-coded)、`Panic → Escalate` に相当する variant なし | `StackOverflowError` 相当の致命的障害が stop になって親に伝播しない。障害の上方伝播契約が破れる | **high** |
| AL-H1 | post_restart hook + preRestart デフォルト実装 | `Actor.preRestart` のデフォルトは `context.children.foreach(stop)` で子を全停止、`postRestart(cause)` → `preStart()` | `actor_lifecycle.rs` — `pre_restart` デフォルトは `post_stop` を呼ぶだけで子を止めない。`post_restart` trait メソッドが存在しない | restart 時の「古い actor の子を全停止」と「新 actor に restart 由来を通知」の両方が欠落 | **high** |
| ES-M1 | subscribe/unsubscribe atomicity | `EventStream.scala` — `AtomicReference` + `@tailrec` CAS で classifier map を lock-free 更新 | `event_stream_shared.rs` — `SharedRwLock<EventStream>` で write lock 取得 | 高頻度 subscribe/unsubscribe で性能差。機能的には等価 | medium |
| FS-M1 | FSM の `forMax` / `replying` | `FSM.scala` — `goto(S).forMax(5.seconds)`, `stay().replying(msg)` | `fsm_transition.rs` — `goto`, `stay`, `stop`, `using` のみ、`forMax`/`replying` なし | 遷移ごとの timeout 上書き、遷移前返信が不可 | medium |
| FS-M2 | FSM の名前付き arbitrary timer | `FSM.scala` — `setTimer(name, msg, duration, repeat)`, `cancelTimer(name)`, `isTimerActive(name)` | `fsm/` — `set_state_timeout` (state-scoped のみ) + `ctx.timers()` 経由、arbitrary name timers なし | state 外の独立タイマー管理ができない | medium |
| SP-M1 | maxNrOfRetries の意味反転 | `FaultHandling.scala` — `maxNrOfRetries = -1` → 無制限、`= 0` → 1 回も retry しない | `base.rs` — `max_restarts = 0` → 無制限 (None 扱い) | Pekko 設定値をそのまま使うと意味が逆転 | medium |
| AL-M1 | post_restart hook 欠落 | `Actor.postRestart(reason)` → `preStart()` | `pre_start` の `LifecycleStage::Restarted` 引数で代替するが、trait メソッドとしては未分離 | restart 由来の初期化とプレーン起動の初期化が分離できない | medium |

### その他 low 判定（約 10 件）

- Mailbox status bit の割付違い（`FLAG_RUNNING` 独立フラグ、`FLAG_FINALIZER_OWNED` / `FLAG_CLEANUP_DONE` 2 段階クローズ）— 挙動差はなし、実装選択
- Stash オーバーフロー例外型の差異（Pekko `StashOverflowException` vs fraktor `ActorError::recoverable(STASH_OVERFLOW_REASON)`）— recovery 可能性の差
- VirtualThread 対応 (`Dispatcher.scala:isVirtualized`) — JVM 固有、対応不要
- ChildNameReserved (children container の名前予約) — 生成レース対策、Rust 所有権で代替可能
- Supervision の `Error → Escalate` 以外の decider 細分化
- FSM `onTransition` で stay() を fire しない契約（fraktor は `explicit_transition=false` 相当で対応済み、挙動一致だが契約明記なし）
- LoggingBus と EventStream の統合スタイル差異（機能等価）

### 内部セマンティクス修正の実装優先度

#### Phase A1（即時対応 / high 影響）

1. **MB-H1 Suspend 時 enqueue 拒否の撤廃** — `enqueue_envelope` から `is_suspended()` チェックを除去。suspend は dequeue 側のみで制御する。`mailbox/base.rs:413-451`
2. **MB-H2 cleanup 時の system queue DeadLetters 転送** — `finalize_cleanup` で `system_queue.drain()` も実施し、各 SystemMessage を DeadLetters へ転送。`mailbox/base.rs:621-637`
3. **MB-H3 bounded overflow 時の DL 通知** — `DropNewest`/`DropOldest` 実行前に `DeadLetter` イベントを publish。`bounded_message_queue.rs:34-57`
4. **AC-H1 user msg 1 件ごとの system msg flush** — `run()` ループを「user 1 件 → system fully drain → suspend チェック → 次の user」構造に変更。user/system キューの分離設計が必要 (large)
5. **SP-H1 Decider の粒度** — `ActorError` に `Panic` variant を追加、default decider で `Panic => Escalate` を返す

#### Phase A2（設計変更を伴う / high 影響）

6. **AC-H2 ChildrenContainer 4 状態 state machine** — `ActorCellState` に `children_state: ChildrenContainerState` を追加、`Normal / Terminating { reason, dying } / Terminated` の 3 状態を実装
7. **AC-H3 faultSuspend/Resume の子再帰伝播** — `SystemMessage::Suspend` / `Resume` 処理内で子全員に再帰送信
8. **AC-H4 restart 中の子停止完了待ち** — AC-H2 と連動、`handle_recreate` を `pre_restart → 子 Stop → 完了待ち → recreate → post_restart → resume` の非同期フローに変更
9. **AC-H5 terminatedQueued 実装** — `handle_terminated` を user メッセージ経由に変更、`terminated_queued: HashSet<Pid>` を `ActorCellState` に追加、`unwatch` 時の取消対応
10. **AL-H1 post_restart hook + preRestart デフォルト実装** — `Actor` trait に `post_restart(&mut self, ctx, reason)` を追加、`pre_restart` デフォルトを「子全停止」に変更

#### Phase A3（medium 影響 / 個別対応）

11-24. medium 項目（MB-M1〜3, AC-M1〜5, ES-M1, FS-M1〜2, SP-M1, AL-M1）— 影響度に応じて個別 PR で対応。特に **MB-M1 throughput deadline** は AC-H1 と同時対応が効率的。

#### Phase A4（low 影響 / 長期整備）

low 判定約 10 件。公開 API には影響せず、優先度は低い。Phase A1-A3 完了後の ergonomic 改善として扱う。

### 内部セマンティクスギャップの検出手法について

第8版の検出手法は以下の 3 エージェント並列比較:

1. **Mailbox セマンティクス比較エージェント** — Pekko `Mailbox.scala` / `Mailboxes.scala` / `UnboundedMailbox.scala` / `BoundedMailbox.scala` / `ControlAwareMailbox.scala` / `SystemMessage.scala` と fraktor-rs `mailbox/` 配下全ファイルを行単位で比較
2. **Dispatcher / ActorCell 比較エージェント** — Pekko `dispatch/Dispatcher*.scala` + `actor/dungeon/` 全ファイル (Children, DeathWatch, Dispatch, FaultHandling, ReceiveTimeout) と fraktor-rs `dispatch/dispatcher/` + `actor/actor_cell/` + `actor/lifecycle/` を比較
3. **EventStream / Scheduler / FSM / Stash / Supervision 比較エージェント** — Pekko `event/EventStream.scala` / `actor/Scheduler.scala` / `actor/FSM.scala` / `actor/Stash.scala` / `actor/SupervisorStrategy.scala` と fraktor-rs `event/` + `actor/scheduler/` + `actor/fsm/` + `actor/supervision/` を比較

各エージェントは **8〜16 観点**で一致判定（完全一致 / 部分一致 / 不一致 / 未実装）と深刻度（low / medium / high）を出力。合計 34 観点。

今後は、**pekko-porting facets の implement ステップ** でこの比較手法を個別バッチに適用し、新規実装のたびに内部セマンティクス突合を必須化する。`.takt/facets/instructions/pekko-porting-implement.md` の Fake Gap チェックはシグネチャ面の偽装検出だが、**内部セマンティクス逸脱は Fake Gap チェックを通過してしまう** ため、別途「Pekko 内部参照の行単位突合」を追加する必要がある（今後の facets 改訂で検討）。

## まとめ

- actor モジュールの parity は **公開 API レベルでは 100%**（101/101 型）に到達。enumerated API gap / 部分実装ギャップともに **0 件**。第7版では pekko-porting Batch 4 の closing を反映し、Phase 3 hard の `OptimalSizeExploringResizer` を classic + typed expose まで一括で実装完了した。
- **しかし第8版で内部セマンティクス観点 34 項目を比較した結果、high 11 / medium 13 / low 約 10 の不一致を検出**。「公開 API 100%」は「Pekko と同じ契約で動いている」ことを保証しない。
- **完全カバー済みカテゴリ**（100%）: classic actor core, supervision, typed core surface, event/logging, receptionist, scheduling/timers, ref/resolution, delivery/pubsub, serialization, extension, coordinated shutdown, pattern, dispatch/mailbox, std adaptor, **classic routing, typed routing** — **16カテゴリ全て**で完全 parity（第6版比 +2: classic routing 14/15 → 15/15, typed routing 6/7 → 7/7）。
- **第7版での主要な前進（Batch 4 closing）**:
  - `OptimalSizeExploringResizer` を **typed DSL 層に 1 つの公開型として翻訳実装**。Pekko 側の classic/typed 二重配置を fraktor-rs の typed DSL 集約方針に合わせ、`PoolRouter::with_resizer` から直接利用可能な形に統合。
  - 3 アクション（downsize / explore / optimize）・10 チューニングパラメータ・`performance_log: BTreeMap<usize, Duration>` による `(size → ms/message)` 記録・`under_utilization_streak` による縮小遅延・`weightedAverage` による安定化を Pekko 契約に沿って全て翻訳。
  - **`Clock: Send + Sync` trait + シード可能 LCG (Numerical Recipes MMIX)** で Pekko の `ThreadLocalRandom` / `System.nanoTime()` 依存を置換し、決定的な explore / optimize 分岐を実現（テスト再現性確保）。
  - **`Resizer` trait 拡張**: `resize(usize) → resize(&[usize])` への署名変更（破壊的だが `DefaultResizer` は `&slice.len()` 利用のみで実害なし）、`report_message_count(&[usize], u64)` default no-op メソッド新規追加。throughput-aware resizer のみ override し、`DefaultResizer` は既存動作を継続。
  - **`PoolRouter` 配線変更**: `observe_routee_mailbox_sizes` ヘルパで `ActorRef::system_state()` → `Mailbox::user_len()` スナップショットを取得し、毎メッセージで `report_message_count`、`is_time_for_resize` 真時に同じスナップショットを `resize` へ渡す（Pekko `ResizablePoolCell.sendMessage` と同等順序・スナップショット共有）。
  - **AShared パターン非採用**: `DefaultResizer` / `CircuitBreaker` の前例踏襲で「1 型 + 内部 `SpinSyncMutex`」パターンを採用。第2層の `*Shared` ラッパーは作らない。`immutability-policy.md` の軽微逸脱理由は `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md` に明記。
  - **センチネル置換**: Pekko の `var checkTime = 0L` は抽象 `Clock::Instant` に対して意味のある 0 値が定義できないため `has_recorded: bool` + `check_time: I` のペアに置換。
  - **no_std 互換性**: `f64::ceil` / `f64::floor` が使えないため `libm_ceil` / `libm_floor` を実装内フリー関数として提供（外部 `libm` クレート依存回避）。
- **判定根拠の保存**: Batch 4 の採用/翻訳/非採用判定、Pekko 契約意図との対応、fraktor-rs 設計ルールとの整合、未来の判定変更トリガは全て `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md` に保存。
- **残存ギャップ**: API parity 観点では **ゼロ**。残る改善余地は **内部モジュール構造の整理** のみ:
  1. receptionist facade / protocol / runtime の `receptionist/` 配下への再配置（medium / high）。
  2. typed delivery の `internal/` 層新設による controller 実装詳細と公開 DTO の分離（medium / medium）。
  3. classic kernel の public surface 縮小（`pub(crate)` 化の継続的推進、medium / medium）。
- これら 3 項目は parity カウント対象外。actor モジュールは **parity 観点で Pekko と完全同等** の状態に到達した。公開境界の厳格さ（`pub(crate)` 化の推進）が進めば、以後は Pekko を上回る状態を目指せる。
