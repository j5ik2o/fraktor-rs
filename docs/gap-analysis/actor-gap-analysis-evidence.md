# actor モジュール ギャップ分析 詳細根拠

この文書は [actor-gap-analysis.md](./actor-gap-analysis.md) の判定根拠を保持する。
現在の意思決定に必要な要約は本体ドキュメントを参照する。

## 比較対象

| 対象 | パス |
|------|------|
| fraktor-rs core/kernel | `modules/actor-core/src/core/kernel/` |
| fraktor-rs core/typed | `modules/actor-core/src/core/typed/` |
| fraktor-rs std adaptor | `modules/actor-adaptor-std/src/std/` |
| Pekko classic | `references/pekko/actor/src/main/scala/org/apache/pekko/` |
| Pekko typed | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` |

## 公開 API カバレッジ根拠

| 層 | Pekko 対応数 | fraktor-rs 実装数 | 判定 |
|----|--------------|-------------------|------|
| core / untyped kernel | 39 | 39 | 完了 |
| core / typed wrapper | 56 | 56 | 完了 |
| std / adaptor | 6 | 6 | 完了 |
| 合計 | 101 | 101 | 完了 |

core / untyped kernel の母数が 40 から 39 に減ったのは、`ConsistentHash<T>` / `MurmurHash` util グループを n/a 化したためである。
rendezvous hashing では ring も MurmurHash util も不要であり、Pekko の契約意図である stable mapping / minimal disruption / hash key precedence / NoRoutee は fraktor-rs の実装で満たしている。

## カテゴリ別判定

| カテゴリ | 判定 | 根拠 |
|----------|------|------|
| classic actor core | 完了 | `Actor`, `ActorCell`, `ActorContext`, `ActorPath`, `ActorRef`, `ActorSelection`, `Props`, `PoisonPill`, `Kill` などを実装済み |
| supervision / fault handling | 完了 | `SupervisorStrategy`, `SupervisorDirective`, `RestartStatistics`, backoff 系を実装済み |
| typed core surface | 完了 | `Behavior`, `ActorContext`, typed `ActorRef`, receptionist, scheduler, message adapter, ask / pipeToSelf などを実装済み |
| dispatch / mailbox | 完了 | dispatcher / mailbox registry、pinned / balancing / affinity、bounded / deque / control-aware を実装済み |
| event / logging | 完了 | `EventStream`, `LoggingAdapter`, marker / MDC, logging filter を実装済み |
| pattern | 完了 | ask, graceful stop, retry, circuit breaker 相当を実装済み |
| classic routing | 完了 | pool / group / router command / consistent hashing / smallest mailbox などを実装済み |
| typed routing | 完了 | typed routing DSL と resizer を実装済み |
| discovery / receptionist | 完了 | typed receptionist facade / runtime を実装済み |
| scheduling / timers | 完了 | classic / typed scheduler と FSM named timer を実装済み |
| ref / resolution | 完了 | ActorRefResolver / path resolution を実装済み |
| delivery / pubsub | 完了 | reliable delivery facade / pubsub を実装済み |
| serialization | 完了 | serializer registry / manifest / extension を実装済み |
| extension | 完了 | ActorSystem extension 登録と lookup を実装済み |
| coordinated shutdown | 完了 | phase / task / reason / timeout を実装済み |
| std adaptor | 完了 | std logging / executor / tick driver / circuit breaker registry を実装済み |

## 内部セマンティクス比較の検出手法

第8版では 3 つの比較観点で Pekko 参照実装と fraktor-rs 実装を突合した。

| 比較観点 | Pekko 側 | fraktor-rs 側 |
|----------|----------|---------------|
| Mailbox | `Mailbox.scala`, `Mailboxes.scala`, `UnboundedMailbox.scala`, `BoundedMailbox.scala`, `ControlAwareMailbox.scala`, `SystemMessage.scala` | `core/kernel/dispatch/mailbox/` |
| Dispatcher / ActorCell | `dispatch/Dispatcher*.scala`, `actor/dungeon/*` | `core/kernel/dispatch/dispatcher/`, `actor/actor_cell/`, `actor/lifecycle/` |
| EventStream / Scheduler / FSM / Stash / Supervision | `event/EventStream.scala`, `actor/Scheduler.scala`, `actor/FSM.scala`, `actor/Stash.scala`, `actor/SupervisorStrategy.scala` | `event/`, `actor/scheduler/`, `actor/fsm/`, `actor/supervision/` |

検出した 34 観点は完全一致 / 部分一致 / 不一致 / 未実装で分類し、深刻度を high / medium / low に分けた。

## 内部セマンティクス ID 一覧

| ID | 観点 | 現在判定 |
|----|------|----------|
| MB-H1 | Suspend 時の enqueue 契約 | done |
| MB-H2 | cleanup 時の system queue DeadLetters 転送 | done |
| MB-H3 | bounded overflow 時の DeadLetters 通知 | done |
| MB-M1 | throughput deadline | done |
| MB-M2 | BoundedDequeBasedMailbox / BoundedControlAwareMailbox | done |
| MB-M3 | blocking push-timeout 戦略 | n/a / design divergence |
| AC-H1 | user msg 1 件ごとの system msg flush | done |
| AC-H2 | ChildrenContainer 状態機械 | done |
| AC-H3 | faultSuspend / faultResume の子再帰 | done |
| AC-H4 | restart 中の子停止完了待ち | done |
| AC-H5 | terminatedQueued による遅延 Terminated delivery | done |
| AC-M1 | PinnedDispatcher の actor 登録排他 | done |
| AC-M2 | Dispatcher alias chain resolution | done |
| AC-M3 | FailedFatally / isFailed guard | done |
| AC-M4a | watchWith 重複チェック | done |
| AC-M4b | address terminated 購読 | deferred |
| AC-M5 | NotInfluenceReceiveTimeout marker | done |
| DP-M1 | Dispatcher primary entry id alignment | done |
| MB-P1 | Mailbox primary entry id alignment | done |
| ES-H1 | EventStream subchannel classifier | done |
| ES-M1 | EventStream subscribe / unsubscribe atomicity | low |
| SP-H1 | decider 粒度と panic supervision 経路 | done |
| SP-H1.5 | std adaptor panic guard | done |
| SP-M1 | maxNrOfRetries semantics | done |
| AL-H1 | post_restart hook + preRestart default | done |
| AL-M1 | post_restart hook 表記整合 | done |
| FS-M1 | FSM `forMax` / `replying` | done |
| FS-M2 | FSM named arbitrary timer | done |

## 残存 medium 詳細

### AC-M4b

Pekko:

- `DeathWatch.scala` では remote address termination を EventStream 経由で購読する。
- remote node 障害時、watched remote actor に対して Terminated を配送する。

fraktor-rs:

- local DeathWatch の watch / unwatch / watchWith 重複チェックと terminated dedup は実装済み。
- remote / cluster transport から actor core DeathWatch へ address terminated 相当を配送する接続が未完了。

判定:

- remote / cluster 基盤に依存するため deferred。
- actor core 単体の high risk ではない。

## n/a / divergence 根拠

| 項目 | 判定理由 |
|------|----------|
| Java DSL (`AbstractActor`, `ReceiveBuilder`, `BehaviorBuilder` など) | JVM / Java 継承モデル依存。Rust では trait / closure / builder で代替 |
| `javadsl/`, `japi/` | Java API interop 層のため対象外 |
| Pekko IO (`Tcp`, `Udp`, `Dns`) | actor core ではなく transport / remote の責務 |
| `JavaSerializer` / `DisabledJavaSerializer` | JVM Java serialization 固有 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | JVM classloader / reflection 固有 |
| HOCON-based dispatcher dynamic loading | HOCON parser と JVM reflection に依存。fraktor-rs では typed factory 登録が等価責務 |
| `VirtualThreadExecutorConfigurator` | JVM / Java 21 virtual thread 固有 |
| `ProviderSelection` | JVM ActorSystem provider 選択機構 |
| `LoggingFilterWithMarker` | `LogEvent` が marker 情報を保持しており既存 `LoggingFilter` で代替可能 |
| `ConsistentHash<T>` / `MurmurHash` / `virtualNodesFactor` | rendezvous hashing 採用により ring 構造が不要 |
| `AtomicReference` routees cache | rendezvous hashing はキャッシュ対象の ring を持たず、内部可変性方針にも合わない |
| `ConsistentRoutee` wrapper | `Routee::ActorRef` が `Pid` を一意識別子として保持するため不要 |
| MB-M3 blocking push-timeout | async Rust では runtime worker blocking が設計上不適切。non-blocking overflow strategy で扱う |

## Low 判定の代表例

low は挙動差ではなく性能・実装方式・表現粒度の差として扱う。
履歴上の件数は比較粒度により揺れるため、ここでは現在の残件判断に影響する代表例だけを残す。

| 項目 | 理由 |
|------|------|
| Mailbox status bit の割付違い | 挙動差ではなく実装選択 |
| Stash overflow error type の差異 | recovery 可能性の表現差 |
| VirtualThread 対応 | JVM 固有 |
| ChildNameReserved | Rust 所有権と registry 設計で代替可能 |
| Supervision decider 細分化 | `Error -> Escalate` 以外は致命差ではない |
| FSM `onTransition` stay 非発火契約 | fraktor は `explicit_transition=false` 相当で挙動一致 |
| LoggingBus と EventStream の統合スタイル差異 | 機能等価 |
| EventStream lock-free CAS 非採用 | 性能差のみ |

## 構造ギャップ

| 構造ギャップ | 状態 | 推奨アクション |
|--------------|------|----------------|
| classic kernel の public surface が広い | 継続 | 利用者向け facade から再公開されない補助型を `pub(crate)` 化する |

第21版で構造ギャップから除外した完了項目:

| 完了項目 | 根拠 |
|----------|------|
| receptionist の facade / protocol / runtime 分離 | runtime は `core/typed/receptionist/runtime.rs` に分離済み |
| typed delivery の internal 層分離 | controller 実装詳細は `core/typed/delivery/internal/` 配下 |
| dispatcher / mailbox factory API 整理 | `MessageDispatcherFactory` / `MailboxFactory` を extension point として整理済み |

## 判定維持ルール

- 公開 API が存在しても、内部セマンティクスが未突合なら parity 完了とは扱わない。
- n/a は「未実装」ではなく、Pekko の実装詳細または JVM 固有性を Rust 設計へそのまま持ち込まない判断である。
- design divergence は実運用の要求または性能測定で問題化した場合のみ再検討する。
- remote / cluster 連携が進んだら、AC-M4b を最優先で再評価する。
