# actor モジュール ギャップ分析

## 前提

- Pekko互換仕様の実現+Rustらしい設計を目指す
- **Pekko互換フェイクはNG**。型名・関数名・シグネチャの存在だけでは「実装済み」と判定しない。状態遷移、失敗経路、監視/再起動、panic 変換、mailbox 契約まで Pekko の意味論と一致して初めて完了とみなす
- 手間が掛かっても、常に本質的な設計の選択肢を選ぶこと
- フォールバックや後方互換姓を保つコードを書かないこと
- modules/*-coreのコアロジックは原則no_stdとする。modules/*-adaptor-stdはstd依存アダプタ実装を配置する。
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
- 分析日: 2026-04-22（初版: 2026-04-15、第2版: 2026-04-16、第3版: 2026-04-17、第4版: 2026-04-17、第5版: 2026-04-17、第6版: 2026-04-17、第7版: 2026-04-18、第8版: 2026-04-19、**第8.1版: 2026-04-19** — Phase A1 完了反映、**第9版: 2026-04-21** — InvokeGuard / PanicInvokeGuard による SP-H1.5 完了反映、**第10版: 2026-04-22** — Phase A2+ (AC-H2/H3/H4/H5/AL-H1/ES-H1) 全完了反映、**第11版: 2026-04-22** — SP-M1 (maxNrOfRetries 意味反転) 完了反映、`RestartLimit` enum + Pekko one-shot window 実装）
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
- **第8.1版での更新 (次回 takt 実行時の skip 対象)**: PR #1594 (branch `impl/pekko-actor-phase-a1-mailbox-semantics`) で **Phase A1 の 5 件（MB-H1 / MB-H2 / MB-H3 / AC-H1 / SP-H1）が完了**。残存内部セマンティクスギャップは **high 6 件 / medium 13 件 / low 約 10 件**。PR review の追加対応として scheduling gate 整合 + DL 二重記録設計の Pekko 完全準拠化（`EnqueueOutcome::Rejected` 新設）も同 PR で解決済み。詳細は「内部セマンティクスギャップ」セクションを参照。**次回の pekko-porting ワークフローでは Phase A1 項目全件 (MB-H1/H2/H3, AC-H1, SP-H1) を skip 対象とすること**。
- **第10版での更新 (2026-04-22)**: Phase A2+ の内部セマンティクス high 項目 **6 件 (AC-H2 / AC-H3 / AC-H4 / AC-H5 / AL-H1 / ES-H1) が全完了**。残存内部セマンティクス high は **0 件**（Phase A1/A2 合計 11 件 + SP-H1.5 を全消化）。内訳 archive:
  - `2026-04-21-2026-04-20-pekko-restart-completion`: AC-H2 (ChildrenContainer 4 状態) / AC-H4 (restart 中の子停止完了待ち) / AC-H5 (terminatedQueued) / AL-H1 (post_restart hook + default pre_restart) を実装（commit `68078f79` + `ff44aee7`）。AC-H3 (faultSuspend/Resume の子再帰) も同時期に `system_invoke` の `Suspend`/`Resume` arm (`actor_cell.rs:1553-1566`) へ `suspend_children` / `resume_children` として配線済み。
  - `2026-04-21-2026-04-20-pekko-eventstream-subchannel`: ES-H1 (EventStream classifier のサブクラス対応) を `classifier_key.rs` + `event_stream_subscriber_entries.rs` ベースで実装。
  - `2026-04-22-pekko-default-pre-restart-deferred` (**本版で反映する直近マージ**): AC-H4 / AL-H1 の sync dispatch 上 deferred ケースを `ExecutorShared::enter_drive_guard` + `DriveGuardToken` RAII + `MessageDispatcherShared::run_with_drive_guard` で閉塞。`fault_recreate` 内の `pre_restart` 呼び出しを guard でラップし、default `pre_restart` の `stop_all_children` が同一スレッド上で子 mailbox を inline drain するのを防ぐ（commit `4978f30b`）。これにより production dispatcher と同期 dispatcher の双方で deferred 契約が成立。
- **第9版での更新 (2026-04-21)**: PR #1602 向けレビュー対応で `InvokeGuard` / `InvokeGuardFactory` / `PanicInvokeGuard` / `PanicInvokeGuardFactory` を再設計し、**SP-H1.5 (std adaptor 層での panic → `ActorError::Escalate` 配線)** を完了扱いに更新した。
  - `modules/actor-adaptor-std/src/std/actor/panic_invoke_guard.rs` で `std::panic::catch_unwind` により `receive` panic を `ActorError::escalate(...)` へ変換。
  - `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` と `.../invoke_guard/invoke_guard_factory.rs` に guard 抽象を導入し、`MessageInvokerPipeline` が常に guard 経由で `receive` を実行。
  - `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/pipeline.rs` では **0 回呼び出し** / **2 回以上呼び出し** の両方を fatal として検出するため、名前だけの panic 互換は成立しない。
  - `PanicInvokeGuardFactory` / `NoopInvokeGuardFactory` は `ArcShared<dyn InvokeGuard>` をキャッシュして clone を返す。
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
| **内部セマンティクスギャップ数 (第9版、SP-H1.5 完了反映後)** | **19+（high 6 / medium 13 / low 約 10）** — Phase A1 の 5 件 (MB-H1/H2/H3, AC-H1, SP-H1) に加え、SP-H1.5 を PR #1602 で完了 |
| **内部セマンティクスギャップ数 (第10版、Phase A2+ 完了反映後)** | **13+（high 0 / medium 13 / low 約 10）** — AC-H2/H3/H4/H5/AL-H1/ES-H1 を完了 |
| **内部セマンティクスギャップ数 (第11版、SP-M1 完了反映後)** | **12+（high 0 / medium 12 / low 約 10）** — SP-M1 (maxNrOfRetries 意味反転 + RestartStatistics one-shot window) を完了 |
| **内部セマンティクスギャップ数 (第12版、MB-M1 完了反映後)** | **11+（high 0 / medium 11 / low 約 10）** — MB-M1 (mailbox throughput deadline enforcement) を完了 |
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
| MB-H1 | ✅ **完了 (PR #1594)** — Suspend 時の enqueue 契約 | `Mailbox.scala:99` — enqueue は常に受理、suspend は dequeue/processing 側で制御 | `mailbox/base.rs:488-496` — `enqueue_envelope` は suspended でも受理（is_closed チェックのみ） | Phase A1 で修正。Pekko 契約に準拠 | ~~high~~ done |
| MB-H2 | ✅ **完了 (PR #1594)** — Cleanup 時の system queue → DeadLetters 転送 | `Mailbox.scala:288-330,338-352` — `cleanUp()` で system / user 両キューを drain して DeadLetters に送る | `mailbox/base.rs:finalize_cleanup` — system queue も drain し DeadLetters へ転送 | Phase A1 で修正 | ~~high~~ done |
| MB-H3 | ✅ **完了 (PR #1594)** — Bounded overflow 時の DeadLetters 通知 | `Mailbox.scala:428-432` — `BoundedNodeMessageQueue.enqueue` が overflow 時に `deadLetters.tell(DeadLetter(...))` を送る | `mailbox/base.rs:enqueue_envelope_locked` — `EnqueueOutcome::{Evicted, Rejected}` 経由で `record_dead_letter(..., DeadLetterReason::MailboxFull, ...)` を実行し、Pekko void-on-success 契約に準拠 (`Ok(())` 返却) | Phase A1 で修正 + refactor (5e6e6f6: Pekko 完全準拠化、`EnqueueOutcome::Rejected` 新設、二重登録設計上不可能に) | ~~high~~ done |
| MB-M1 | ✅ **完了 (change `pekko-mailbox-throughput-deadline`)** — Throughput deadline | `Mailbox.scala:261-278` — `System.nanoTime` ベースで deadline 超過を検知して即 reschedule | `mailbox/base.rs:run()` で `MailboxClock` (`ArcShared<dyn Fn() -> Duration + Send + Sync>`) を `MailboxSharedSet` 経由で注入、`deadline_at = clock.zip(throughput_deadline).map((c, d) → c().saturating_add(d))` を run 先頭で 1 回計算、`process_mailbox` ループ内で `invoke → process_all_system_messages → left -= 1 → if clock() >= da break` (Pekko `Mailbox.scala:271-276` 行単位対応)。std adaptor で `Instant::now()` capture 起点の closure を `ActorSystem` 初期化時に install | ~~medium~~ done |
| MB-M2 | BoundedDequeBasedMailbox / BoundedControlAwareMailbox | `Mailbox.scala:844,931` — 両型あり | 両型とも **未実装** | Stash + bounded の組み合わせ、Control-aware + bounded の組み合わせが選択不可 | medium |
| MB-M3 | Blocking push-timeout 戦略 | `Mailbox.scala:556-566` — `pushTimeOut` 経由で `queue.offer(handle, timeout)` により送信側を block | 未実装（非同期 Rust で blocking は設計外） | Rust 設計上は妥当だが、Pekko ユーザーの backpressure 期待値とは異なる | medium |

### Dispatcher / ActorCell 系（5 high / 5 medium）

| ID | 観点 | Pekko (ファイル:行) | fraktor-rs (ファイル:行) | 不一致の内容 | 深刻度 |
|----|------|--------------------|------------------------|-------------|--------|
| AC-H1 | ✅ **完了 (PR #1594)** — user msg 1 件ごとの system msg flush | `Mailbox.scala:274` — `processMailbox()` の再帰ループ内で user msg 1 件処理後に必ず `processAllSystemMessages()` を呼ぶ | `mailbox/base.rs:run()` — 初回 system drain → 毎 user msg 処理後 system drain + suspend チェック → 末尾 system drain に再構造化。throughput カウンタは user 専用に純化 (system は budget 消費しない) | Phase A1 で修正。Pekko `processAllSystemMessages()` 再帰呼び出しと等価なセマンティクス | ~~high~~ done |
| AC-H2 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-restart-completion`)** — ChildrenContainer の状態機械 | `dungeon/ChildrenContainer.scala` — `EmptyChildrenContainer` / `NormalChildrenContainer` / `TerminatingChildrenContainer` / `TerminatedChildrenContainer` + `SuspendReason(Recreation/Creation/Termination)` | `children_container.rs` + `suspend_reason.rs` に `Normal / Terminating { reason, dying } / Terminated` の 3 状態 + `SuspendReason::Recreation` を実装。`ActorCellState::children_state` で保持（commit `ff44aee7` + `68078f79`） | Pekko `Empty` は Rust の `Normal { children: [] }` へ統合し、state 遷移契約は Pekko と同等 | ~~high~~ done |
| AC-H3 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-restart-completion`)** — faultSuspend / faultResume の子再帰 | `dungeon/FaultHandling.scala:124-153` — `suspendNonRecursive` + `suspendChildren` の pair、`resumeNonRecursive` + `resumeChildren` で子全員に再帰 | `actor_cell.rs:1553-1566` — `SystemMessage::Suspend` / `Resume` arm で `suspend_children()` / `resume_children()` を呼び、AC-H3 recursion をコメント明記で配線 | mailbox 層の counter 更新は MB-H1 側が担い、arm は子への propagation のみ行う | ~~high~~ done |
| AC-H4 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-restart-completion` + `2026-04-22-pekko-default-pre-restart-deferred`)** — restart 中の子停止完了待ち | `FaultHandling.scala:100-118` — `faultRecreate` → `aroundPreRestart` → `setChildrenTerminationReason(Recreation)` → (全子 Terminated 受信) → `finishRecreate` → `aroundPostRestart` → mailbox.resume | `actor_cell.rs` に `fault_recreate(cause)` / `finish_recreate(cause)` の 2 フェーズを実装。`set_children_termination_reason(SuspendReason::Recreation(cause))` で deferred=true 判定、最終 child の `DeathWatchNotification` 受信時に `handle_death_watch_notification` から `finish_recreate` 駆動。**第10版で反映**: default `pre_restart` の `stop_all_children` が同期 dispatcher 上で子 mailbox を inline drain する再入問題を `MessageDispatcherShared::run_with_drive_guard` (既存 `ExecutorShared` トランポリンの `running: AtomicBool` を RAII で外部 claim) でラップして解消（commit `4978f30b`） | production async / 同期 inline の両 dispatcher 上で deferred 契約が成立 | ~~high~~ done |
| AC-H5 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-restart-completion`)** — terminatedQueued による遅延 Terminated delivery | `dungeon/DeathWatch.scala:32,111` — `terminatedQueued: Map[ActorRef, Option[Any]]` に記録して user キュー経由で delivery、`unwatch` で `terminatedQueued -= a` で取消 | `ActorCellState::terminated_queued: HashSet<Pid>` に dedup マーカーを保持。`SystemMessage::DeathWatchNotification(pid)` が kernel 内で `watching` 判定 + `terminated_queued` dedup を通過した場合のみ `actor.on_terminated` を直接呼び、`unwatch` 時に `terminated_queued` から該当 pid を除去（commit `1c69e803` で stop_all_children 時の dedup marker 保全を後追い修正） | `SystemMessage::Terminated` variant は削除、伝搬経路は `DeathWatchNotification` に一本化 | ~~high~~ done |
| AC-M1 | PinnedDispatcher の actor 登録排他チェック | `PinnedDispatcher.scala:48-53` — `if ((actor ne null) && actorCell != actor) throw` で複数 actor の共有を禁止 | `pinned_executor.rs` — 排他ガードなし、複数 actor が同じ dispatcher を resolve するとスレッドが共有される | pinned 契約「1 actor 1 thread」が破れる可能性 | medium |
| AC-M2 | Dispatcher の config 動的ロード + alias 解決 | `Dispatchers.scala:159-180` — HOCON `type` キーから動的ロード、`Dispatchers.scala:176` で id chain の再帰解決 | `dispatchers.rs:118` — `register()` 経由の静的登録のみ、alias 連鎖解決なし | `dispatcher.foo = "akka.actor.default-dispatcher"` のような設定が動かない | medium |
| AC-M3 | FailedFatally / isFailed ガード | `FaultHandling.scala:73-74` — `isFailed` で重複 fail 抑制、`isFailedFatally` で以降の復旧を完全拒否 | なし | 失敗中に同一 actor へ複数 fail が来た時の競合や、fatal 障害の永続マーキングが欠落 | medium |
| AC-M4 | watchWith 重複チェック + address terminated 購読 | `DeathWatch.scala:40,127-132` — `maintainAddressTerminatedSubscription` でノード障害通知を購読、`checkWatchingSame` で重複 watchWith を `IllegalStateException` で拒否 | なし | クラスタ環境でのノード障害通知が受信できず、`watchWith(a, M1)` と `watchWith(a, M2)` の二重登録が静かに発生 | medium |
| AC-M5 | NotInfluenceReceiveTimeout マーカー | `ReceiveTimeout.scala:72` — `NotInfluenceReceiveTimeout` 実装メッセージは timeout を reset しない (`ReceiveTimeout` 自身も含む) | `actor_cell.rs:1087` — user msg 成功後は常に `reschedule_receive_timeout()` | `ReceiveTimeout` 自身が届いた時に timeout が自己リセットされ、二重発火の可能性 | medium |

### EventStream / Scheduler / FSM / Stash / Supervision 系（3 high / 5 medium）

| ID | 観点 | Pekko (ファイル:行) | fraktor-rs (ファイル:行) | 不一致の内容 | 深刻度 |
|----|------|--------------------|------------------------|-------------|--------|
| ES-H1 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-eventstream-subchannel`)** — Classifier (サブクラス関係) | `EventBus.scala` — `SubchannelClassification` + `isAssignableFrom` で型ヒエラルキー fan-out (`Animal` 購読で `Dog` も受信) | `classifier_key.rs` + `event_stream_subscriber_entries.rs` + `base.rs` に subchannel 対応 classifier を実装。`EventStreamEvent` は引き続き closed enum だが `ClassifierKey` で階層購読を表現 | Rust は JVM の `Class.isAssignableFrom` 相当を持たないため `ClassifierKey` で契約を等価に再現 | ~~high~~ done |
| SP-H1 | ✅ **完了 (PR #1594 + PR #1602)** — Decider の粒度 (JVM Error → Escalate) と std panic 配線 | `SupervisorStrategy.scala:defaultDecider` — `Error` (JVM fatal) は必ず `Escalate`。dispatcher 側は例外を supervision 経路へ流す | `error/actor_error.rs` — `ActorError::Escalate(ActorErrorReason)` variant + `ActorError::escalate()`。`supervision/base.rs` の 3 deciders に `Escalate => Escalate` arm。さらに `actor-adaptor-std/src/std/actor/panic_invoke_guard.rs` + `MessageInvokerPipeline` guard 配線で std 利用時の `receive` panic を `ActorError::Escalate` へ変換 | Phase A1 で decider を、PR #1602 で **SP-H1.5** を完了。現在は std adaptor 利用時に panic が supervision 経路へ入る | ~~high~~ done |
| AL-H1 | ✅ **完了 (archive `2026-04-21-2026-04-20-pekko-restart-completion` + `2026-04-22-pekko-default-pre-restart-deferred`)** — post_restart hook + preRestart デフォルト実装 | `Actor.preRestart` のデフォルトは `context.children.foreach(stop)` で子を全停止、`postRestart(cause)` → `preStart()` | `Actor::pre_restart` デフォルトを `ctx.stop_all_children()` + `self.post_stop(ctx)` に変更し、`Actor::post_restart(&mut self, &mut ctx, &ActorErrorReason)` trait メソッドを新設（デフォルト `pre_start` 委譲）。sync dispatch 上の deferred ケースは第10版の `run_with_drive_guard` で閉塞 | typed 側 `TypedActor::post_restart` の二重 `pre_start` 問題も commit `984e3cf2` で解消済み | ~~high~~ done |
| ES-M1 | subscribe/unsubscribe atomicity | `EventStream.scala` — `AtomicReference` + `@tailrec` CAS で classifier map を lock-free 更新 | `event_stream_shared.rs` — `SharedRwLock<EventStream>` で write lock 取得 | 高頻度 subscribe/unsubscribe で性能差。機能的には等価 | medium |
| FS-M1 | FSM の `forMax` / `replying` | `FSM.scala` — `goto(S).forMax(5.seconds)`, `stay().replying(msg)` | `fsm_transition.rs` — `goto`, `stay`, `stop`, `using` のみ、`forMax`/`replying` なし | 遷移ごとの timeout 上書き、遷移前返信が不可 | medium |
| FS-M2 | FSM の名前付き arbitrary timer | `FSM.scala` — `setTimer(name, msg, duration, repeat)`, `cancelTimer(name)`, `isTimerActive(name)` | `fsm/` — `set_state_timeout` (state-scoped のみ) + `ctx.timers()` 経由、arbitrary name timers なし | state 外の独立タイマー管理ができない | medium |
| SP-M1 | ✅ **完了 (change `pekko-supervision-max-restarts-semantic`)** — maxNrOfRetries の意味反転 | `FaultHandling.scala:56-86` — `maxNrOfRetries = -1` → 無制限、`= 0` → 1 回も retry しない、one-shot window | `supervision/restart_limit.rs` に `RestartLimit { Unlimited, WithinWindow(u32) }` を新設し、`SupervisorStrategy::max_restarts: RestartLimit` へ置換。`RestartStatistics` も Pekko `ChildRestartStats` / `retriesInWindowOkay` と行単位一致の one-shot window 実装に書き直し (`request_restart_permission(now, limit, window) -> bool`)。typed 層は `with_limit(u32, Duration)` + `with_unlimited_restarts(Duration)` に分解、`i32 + -1 magic + 0 panic` を廃止 | ~~medium~~ done |
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

#### Phase A1（即時対応 / high 影響）✅ **全 5 項目 完了 (PR #1594, branch `impl/pekko-actor-phase-a1-mailbox-semantics`)**

1. ✅ **MB-H1 Suspend 時 enqueue 拒否の撤廃** — `enqueue_envelope` から `is_suspended()` チェックを除去。suspend は dequeue 側のみで制御する。`mailbox/base.rs:488-496`
2. ✅ **MB-H2 cleanup 時の system queue DeadLetters 転送** — `finalize_cleanup` で `system_queue.drain()` も実施し、各 SystemMessage を DeadLetters へ転送
3. ✅ **MB-H3 bounded overflow 時の DL 通知** — `EnqueueOutcome::{Evicted, Rejected}` 経由で mailbox 層が唯一の DL 記録源となり Pekko void-on-success 契約に完全準拠（PR review で Pekko 完全準拠化を実施、commit `5e6e6f6`）
4. ✅ **AC-H1 user msg 1 件ごとの system msg flush** — `run()` ループを「初回 system drain → 毎 user 処理後 system drain + suspend チェック → 末尾 system drain」構造に変更。throughput カウンタは user 専用に純化（system は budget 消費しない）
5. ✅ **SP-H1 Decider の粒度** — `ActorError::Escalate(ActorErrorReason)` variant を追加。3 deciders (`with_decider::default_decider`, `Default::decider`, `backoff_decide`) で `Escalate => Escalate` arm 追加。`FailureClassification::Escalate` で round-trip 保持

> 追加対応（PR #1594 review で発見、同 PR で解決）:
> - **Mailbox scheduling gate 不整合** (`can_be_scheduled_for_execution`): MB-H1 で suspended 時の enqueue を受理するようにした結果、`can_be_scheduled_for_execution` が suspended を全面拒否するのと矛盾 → Pekko `Mailbox.canBeScheduledForExecution` (Mailbox.scala:148-155) に合わせ、suspended 時は system work (hint or `system_len() > 0`) があれば schedulable に修正（commit `ffb7147`）。
> - **DL 二重記録経路の設計ゴミ**: MB-H3 の初回実装は `SendError::Full` を propagate していたため上流 `try_tell::record_send_error` が再記録する経路があった → Pekko `BoundedMailbox.enqueue` の void-on-success 契約に完全準拠化し、mailbox 層を唯一の DL 記録源とする型レベル保証に変更（commit `5e6e6f6`、`EnqueueOutcome::Rejected` 新設 + 各 bounded queue impl を `Ok(Rejected)` へ統一）。
>
> 第9版で完了:
> - **SP-H1.5 (std adaptor 層)**: `PanicInvokeGuard` + `InvokeGuardFactory` + `MessageInvokerPipeline` 配線により、ユーザーハンドラ内 panic の自動 `ActorError::Escalate` 変換を実装済み。
>   - **変更先**: `modules/actor-adaptor-std/src/std/actor/` と `modules/actor-core/src/core/kernel/actor/invoke_guard*`
>   - **完了条件**:
>     - `receive` panic が `ActorError::Escalate` に変換される
>     - no_std core に `std::panic` を持ち込まない
>     - guard が `receive` を 0 回または複数回呼ぶ偽互換を `fatal` で拒否する
>   - **検証**: `modules/actor-adaptor-std/tests/sp_h1_5_panic_guard.rs`, `sp_h1_5_system_escalation.rs`, `modules/actor-core/tests/invoke_guard.rs`
>
> 第10版 (2026-04-22) で Phase A2+ の残り 6 件 (AC-H2 / AC-H3 / AC-H4 / AC-H5 / AL-H1 / ES-H1) が全完了。詳細は以下の Phase A2 セクションと冒頭の第10版更新ノートを参照。

#### Phase A2（設計変更を伴う / high 影響）✅ **全 5 項目 完了 (archives `2026-04-21-2026-04-20-pekko-restart-completion` + `2026-04-22-pekko-default-pre-restart-deferred`)**

6. ✅ **AC-H2 ChildrenContainer 状態機械** — `children_container.rs` + `suspend_reason.rs` に `Normal / Terminating { reason, dying } / Terminated` + `SuspendReason::Recreation` を実装し `ActorCellState::children_state` で保持
7. ✅ **AC-H3 faultSuspend/Resume の子再帰伝播** — `SystemMessage::Suspend` / `Resume` arm (`actor_cell.rs:1553-1566`) で `suspend_children()` / `resume_children()` を呼び、AC-H3 recursion を配線
8. ✅ **AC-H4 restart 中の子停止完了待ち** — `fault_recreate(cause)` / `finish_recreate(cause)` の 2 フェーズを `ActorCell` に実装。最終 child の `DeathWatchNotification` 受信で `handle_death_watch_notification` から `finish_recreate` が駆動される deferred フロー。sync dispatch 上の default `pre_restart` 再入問題は `MessageDispatcherShared::run_with_drive_guard` (第10版) で閉塞
9. ✅ **AC-H5 terminatedQueued 実装** — `ActorCellState::terminated_queued: HashSet<Pid>` に dedup マーカーを保持、`SystemMessage::DeathWatchNotification` が kernel 内で `watching` 判定 + dedup を通過したときのみ `actor.on_terminated` を直接呼ぶ。`SystemMessage::Terminated` variant は削除し伝搬経路を一本化
10. ✅ **AL-H1 post_restart hook + preRestart デフォルト実装** — `Actor::pre_restart` デフォルトを `ctx.stop_all_children()` + `self.post_stop(ctx)` に変更。`Actor::post_restart(&mut self, &mut ctx, &ActorErrorReason)` trait メソッドを新設（デフォルト `pre_start` 委譲）

追加で ES-H1 (EventStream subchannel classifier) も `2026-04-21-2026-04-20-pekko-eventstream-subchannel` で完了済み。これにより Phase A1 / A2 / A2+ の内部セマンティクス high は **全 11 件 + SP-H1.5 がクローズ**し、残存 high は **0 件**となる。

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

- actor モジュールは **主要公開契約リスト上では 101/101 型** に到達しているが、**これは parity 完了を意味しない**。この数値は「表に載せた主要公開契約が存在する」ことだけを示す。
- **第8版で内部セマンティクス観点 34 項目を比較した結果、high 11 / medium 13 / low 約 10 の不一致を検出**。第9版で SP-H1.5 (std panic → Escalate) を閉塞、**第10版で Phase A2+ の high 6 件 (AC-H2 / AC-H3 / AC-H4 / AC-H5 / AL-H1 / ES-H1) を全完了**、**第11版で SP-M1 (maxNrOfRetries 意味反転 + RestartStatistics one-shot window) を完了**、**第12版で MB-M1 (mailbox throughput deadline enforcement) を完了**。結果として残存内部セマンティクスは **high 0 / medium 11 / low 約 10** となり、high 項目は全消化。Pekko互換フェイクを禁じる前提でも、**high レベルの内部セマンティクス gap は閉塞済み**。残り medium / low は個別 PR で対応予定。
- **完全カバー済みカテゴリ**（100%）: classic actor core, supervision, typed core surface, event/logging, receptionist, scheduling/timers, ref/resolution, delivery/pubsub, serialization, extension, coordinated shutdown, pattern, dispatch/mailbox, std adaptor, **classic routing, typed routing** — **16カテゴリ全て**で完全 parity（第6版比 +2: classic routing 14/15 → 15/15, typed routing 6/7 → 7/7）。
- **第9版での主要な前進**:
  - `PanicInvokeGuard` / `InvokeGuardFactory` / `MessageInvokerPipeline` により **SP-H1.5 (std panic → escalate)** を完了。
  - `ArcShared<dyn InvokeGuard>` 化と factory キャッシュ化で、panic 互換実装をホットパス上の常用構成として成立させた。
  - guard が `receive` を呼ばない / 複数回呼ぶケースも fatal として落とすため、名前だけの「互換済み」ではなく契約違反を観測可能にした。
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
- **第10版時点の残存ギャップ**: 公開 API / 内部セマンティクスとも **high 0 件**。残るのは medium 13 件 (MB-M1〜3, AC-M1〜5, ES-M1, FS-M1〜2, SP-M1, AL-M1) と low 約 10 件で、いずれも単発の挙動差または実装選択レベル。`ChildrenContainer` 状態機械, `faultSuspend/faultResume` 子再帰, restart 中の子停止完了待ち (sync/async 両 dispatcher), `terminatedQueued`, `post_restart` 契約, EventStream subchannel classifier はすべて閉塞済み。
- API ギャップと high 内部セマンティクス gap の両方が解消したため、**次のボトルネックは明確に内部構造と medium 項目の個別対応に移る**。構造面 (後述 3 項目) と medium 個別 PR を並行で進める段階。
- **構造面の改善候補**:
  1. receptionist facade / protocol / runtime の `receptionist/` 配下への再配置（medium / high）。
  2. typed delivery の `internal/` 層新設による controller 実装詳細と公開 DTO の分離（medium / medium）。
  3. classic kernel の public surface 縮小（`pub(crate)` 化の継続的推進、medium / medium）。
- これら 3 項目は parity カウント対象外だが有効な整備候補。**第10版時点では high 内部セマンティクス gap はすべて閉塞しており**、残るは medium / low 項目と構造整理のみ。ただし medium 項目にも Pekko 設定値が逆転する `SP-M1` (maxNrOfRetries = 0 の意味) や `NotInfluenceReceiveTimeout` マーカーなど挙動に直結するものが含まれるため、**完全 parity 到達には Phase A3 (medium) の個別対応を要する**。型面の充足と実行時契約の一致は別物であるという第8版以降の立場は維持する。
