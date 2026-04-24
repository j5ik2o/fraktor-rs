# actor モジュール ギャップ分析 更新履歴

この文書は [actor-gap-analysis.md](./actor-gap-analysis.md) から分離した履歴ログである。
現在の残件判断は本体ドキュメントを参照する。

## 版履歴

| 版 | 日付 | 主な更新 | 残存 medium |
|----|------|----------|-------------|
| 初版 | 2026-04-15 | actor / actor-typed / actor-adaptor-std の公開 API 対応状況を初回集計 | 未確定 |
| 第2版 | 2026-04-16 | Java DSL / IO / JVM 固有 API の n/a 判定を整理 | 未確定 |
| 第3版 | 2026-04-17 | Pekko 側を再抽出し、ergonomics 系 API と classic 補助パターンを追加検出 | 未確定 |
| 第4版 | 2026-04-17 | `SmallestMailboxRoutingLogic` の Pekko 互換化を反映 | 未確定 |
| 第5版 | 2026-04-17 | `LoggingFilter` / `RouterConfig` / `AffinityExecutor` の実装済み確認を反映 | 未確定 |
| 第6版 | 2026-04-17 | Batch 1-3 closing を反映。`ConsistentHashableEnvelope`、Listeners、LoggerOps、ConsistentHashingRoutingLogic を整理 | 2 |
| 第7版 | 2026-04-18 | Batch 4 closing。`OptimalSizeExploringResizer` を typed DSL に翻訳実装し、公開 API ギャップ 0 へ到達 | 0 |
| 第8版 | 2026-04-19 | 公開 API カバレッジだけでは parity 完了とみなせないと判断し、内部セマンティクスギャップを追加検出 | 13 |
| 第8.1版 | 2026-04-19 | PR #1594 の Phase A1 完了を反映 | 13 |
| 第9版 | 2026-04-21 | `InvokeGuard` / `PanicInvokeGuard` による SP-H1.5 完了を反映 | 13 |
| 第10版 | 2026-04-22 | Phase A2+ high 項目を全完了。high 0 へ到達 | 13 |
| 第11版 | 2026-04-22 | SP-M1 (`maxNrOfRetries` semantics) 完了を反映 | 12 |
| 第12版 | 2026-04-22 | MB-M1 (mailbox throughput deadline) 完了を反映 | 11 |
| 第13版 | 2026-04-22 | AC-M5 (NotInfluenceReceiveTimeout) 完了を反映 | 10 |
| 第14版 | 2026-04-23 | AC-M1 / AC-M3 完了を反映 | 8 |
| 第15版 | 2026-04-23 | AC-M4a / AL-M1 完了を反映 | 7 |
| 第16版 | 2026-04-23 | MB-M3 を n/a 化、ES-M1 を low 降格 | 5 |
| 第17版 | 2026-04-23 | MB-M2 完了を反映。bounded deque / control-aware mailbox を valid 化 | 4 |
| 第18版 | 2026-04-23 | AC-M2 (dispatcher alias chain) 完了を反映。HOCON dynamic loading を n/a 確定 | 4 |
| 第19版 | 2026-04-23 | DP-M1 / MB-P1 完了。`"default"` legacy token を完全退役 | 3 |
| 第20版 | 2026-04-23 | `pekko-dead-code-retire-phase1` による中途半端な実装残骸 16 件削除を反映 | 3 |
| 第21版 | 2026-04-24 | 現行コード再抽出。receptionist / delivery internal 分離と factory API 整理を反映 | 3 |
| 第22版 | 2026-04-24 | `pekko-fsm-transition-extensions` により FS-M1 / FS-M2 完了 | 1 |

## 主なマイルストーン

### 公開 API カバレッジ

第7版で actor モジュールの主要公開 API は 101/101 に到達した。
ただし第8版で、これは型と関数シグネチャの存在を示すだけで、実行時契約の一致を保証しないと判断した。

### 内部セマンティクス比較

第8版では Mailbox / Dispatcher / ActorCell / ChildrenContainer / FaultHandling / DeathWatch / ReceiveTimeout / ActorLifecycle / EventStream / FSM / Stash / SupervisorStrategy の 34 観点を Pekko 参照実装と比較した。
初回検出は high 11 / medium 13 / low 約 10 だった。

### High 項目の閉塞

第9版から第10版にかけて、SP-H1.5 と Phase A2+ を含む high 項目を全て閉塞した。
第10版以降、内部セマンティクス high は 0 件で推移している。

### Medium 項目の閉塞

第11版から第22版にかけて、supervision、mailbox deadline、receive timeout marker、dispatcher alias、primary id、FSM transition / timer の medium 項目を順次閉塞した。
第22版時点の残存 medium は AC-M4b のみである。

## 完了済み change / archive

| 対象 | change / archive |
|------|------------------|
| Phase A1 mailbox semantics | PR #1594 / branch `impl/pekko-actor-phase-a1-mailbox-semantics` |
| SP-H1.5 panic guard | PR #1602 |
| AC-H2 / AC-H4 / AC-H5 / AL-H1 | `2026-04-21-2026-04-20-pekko-restart-completion` |
| ES-H1 | `2026-04-21-2026-04-20-pekko-eventstream-subchannel` |
| default pre_restart deferred | `2026-04-22-pekko-default-pre-restart-deferred` |
| SP-M1 | `2026-04-21-pekko-supervision-max-restarts-semantic` |
| MB-M1 | `2026-04-22-pekko-mailbox-throughput-deadline` |
| AC-M5 | `2026-04-22-pekko-receive-timeout-not-influence` |
| AC-M1 / AC-M3 | `2026-04-23-pekko-fault-dispatcher-hardening` |
| AC-M4a | `pekko-death-watch-duplicate-check` |
| MB-M2 | `pekko-bounded-deque-control-aware-mailbox` |
| AC-M2 | `pekko-dispatcher-alias-chain` |
| DP-M1 / MB-P1 | `pekko-dispatcher-primary-id-alignment` |
| dead code retire phase 1 | `pekko-dead-code-retire-phase1` |
| FS-M1 / FS-M2 | `pekko-fsm-transition-extensions` |

## 第20版の残骸削除

`pekko-dead-code-retire-phase1` では actor-core の `#[allow(dead_code)]` 46 箇所を監査し、production / test 双方から参照ゼロの 16 項目を削除した。
Pekko 互換機能は `SystemStateShared` や `ExtendedActorSystem` などの生きている経路で成立していたため、削除による機能退行はない。

削除対象の代表例:

- `ActorRefProviders::{contains_key, values}`
- `Cells::{contains, len, is_empty}`
- `ActorSystem::{register_temp_actor, unregister_temp_actor, temp_actor}`
- `SystemState::{handle_failure, suspend_for_escalation, stop_actor}`
- `ConsumerControllerCommand` / `ProducerControllerCommand` の未使用 constructor
- `TypedScheduler::inner`
- `Scheduler::raw`
- `TypedActorSystem::spawn` の untyped path alias

## 第21版の現行コード再抽出

第21版では公開 API と内部セマンティクス件数に変化はなかった。
更新点は構造面の closing である。

- `core/typed/receptionist.rs` は wiring と再公開に限定され、runtime は `core/typed/receptionist/runtime.rs` に分離済み。
- typed delivery は `core/typed/delivery/internal/` に controller 実装詳細を退避し、`delivery.rs` は公開 facade と再公開に限定。
- dispatcher は `MessageDispatcherFactory`、mailbox は `MailboxFactory` を extension point として整理済み。

## 第22版の現状

第22版で FSM `forMax` / `replying` と名前付き timer を完了した。
現時点の残件は以下に集約される。

- medium parity gap: AC-M4b
- structure improvement: classic kernel public surface 縮小
- n/a divergence: MB-M3 producer backpressure
- low performance gap: ES-M1 EventStream 更新方式
