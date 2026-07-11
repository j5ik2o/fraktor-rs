# RFC pekko-0003: dispatch と executor（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/{AbstractDispatcher,Dispatcher,PinnedDispatcher,BalancingDispatcher,BatchingExecutor}.scala`, `actor/ActorRef.scala`, `actor/dungeon/Dispatch.scala`, `actor/src/main/resources/reference.conf` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0003](../0003-actor-dispatch-and-executor.md) |
| 最終照合日 | 2026-07-11 |

## 1. 規範仕様

### 1.1 tell

- **PDISP-1.** `!`（tell）は implicit sender（actor 内では `implicit val self`）を伴う fire-and-forget であり、確認応答・再送機構を持たない（at-most-once。実装挙動からの帰結で、明示的な文言はコードにない）。sender 未指定（`noSender = null`）の場合、`Envelope` 構築時に `system.deadLetters` が sender として埋められる。null メッセージは `InvalidMessageException`。
- **PDISP-2.** 終了済み actor への tell: `unregister` が mailbox を deadLetterMailbox にスワップ済みのため、enqueue は deadLetterMailbox の特殊 `MessageQueue` に入り、`DeadLetter` イベントとして eventStream へ publish される（`DeadLetter` 自身の再帰は抑止）。

### 1.2 dispatch と実行登録

- **PDISP-3.** `Dispatcher.dispatch` は「mailbox へ enqueue → `registerForExecution(hint=message)`」を単一メソッドで行う。fraktor の二段階送信（sender ロック外での登録）に相当する分離は存在しない（JVM 側には per-actor sender ロックがないため不要）。
- **PDISP-4.** `registerForExecution` は `canBeScheduledForExecution`（Open/Scheduled: ヒントまたは実キュー状態 / Closed: 常に false / suspend 中: システムメッセージのみ）→ `setAsScheduled()` CAS → `executorService.execute(mbox)` の順。`RejectedExecutionException` は 1 回だけ再試行し、失敗すれば `setAsIdle()` でロールバックして再送出する。
- **PDISP-5.** mailbox の `run()` は finally で `setAsIdle()` 後に**自分を無条件再登録**する（実キュー状態で judged）。suspend 解除（`resume`）もカウントが 0 に戻った場合に再登録する。
- **PDISP-6.** `inhabitants`（登録 actor 数 + 実行中タスク数）が 0 以下になると shutdown が遅延スケジュールされる。状態は `UNSCHEDULED / SCHEDULED / RESCHEDULED` の 3 状態 CAS（実行時の再増加は RESCHEDULED で再スケジュール。「Warning, racy」コメントあり）。負値は `IllegalStateException("ACTOR SYSTEM CORRUPTED!!!")`。

### 1.3 dispatcher 実装

| 実装 | 特性 |
|------|------|
| `Dispatcher`（既定） | 共有 executor。`throughput = 5` / `throughput-deadline-time = 0ms`（無効）/ `shutdown-timeout = 1s` が既定 |
| `PinnedDispatcher` | `throughput = Int.MaxValue` / deadline = Zero に固定、1 スレッドプール（core=max=1）。別 actor の register は `IllegalArgumentException` |
| `BalancingDispatcher`（deprecated、BalancingPool 推奨） | 全員が単一共有キューを `SharingMailbox` として包む。dispatch は enqueue 後に受信者の登録を試み、失敗時に `teamWork()` が team を走査して他メンバーへ登録を試みる（work donating）。unregister 時も残作業を teamWork で再配布 |

- **PDISP-7.** 共有 mailbox は `MultipleConsumerSemantics` を要求する。`SingleConsumerOnlyUnboundedMailbox` / `NonBlockingBoundedMailbox` は BalancingPool で使用できない（MUST NOT）。

### 1.4 executor

- **PDISP-8.** executor は設定キー `executor` で選択する: `default-executor`（外部 ExecutionContext があればそれ、なければ fallback = fork-join）/ `fork-join-executor`（parallelism 8..64, factor 1.0）/ `thread-pool-executor` / `affinity-pool-executor`（worker 毎の有界 MPSC queue 512、`ThrowOnOverflowRejectionHandler`）/ `virtual-thread-executor` / 任意 FQCN。
- **PDISP-9.** 再入・スタック安全性は `BatchingExecutor` が担う: batchable な Runnable（主に Future コールバック）はスレッドローカルのバッチにまとめられ、ネストした execute は外側バッチ内で逐次処理される（トランポリン）。バッチ内タスクが `scala.concurrent.blocking` に入る場合は `BlockableBatch` が残タスクを executor へ再提出してからブロックする（スタベーション回避）。例外時も未処理分を再提出してから再送出する。

## 2. 不変条件

- **INV-PDISP-1**: mailbox が実行キューに二重投入されることはない（`Scheduled` ビット CAS により成立）。
- **INV-PDISP-2**: enqueue されたメッセージは、mailbox が Closed にならない限り、いつかの `run()` で観測される（finally の無条件再登録 + resume 時再登録により成立）。
- **INV-PDISP-3**: `PinnedDispatcher` の同時所有者は高々 1（違反は例外）。
- **INV-PDISP-4**: バッチ実行中のブロッキング呼び出しが同一スレッド上の残バッチを飢餓させることはない（BlockableBatch の再提出）。

## 3. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| 送信の段数 | dispatch = enqueue + 登録を一体で実行 | 二段階送信（enqueue と登録を per-actor sender ロックの内外に分離。inline executor での再入デッドロック防止） |
| 再スケジュール | run() の finally で無条件に再登録を試みる | mailbox が `need_reschedule` を返し、dispatcher クロージャが再登録（登録漏れを状態で防ぐ） |
| トランポリン | Future コールバック向け `BatchingExecutor`（ThreadLocal バッチ + blocking 検出） | すべての executor タスク向け `ExecutorShared`（drain owner CAS）+ `DriveGuardToken` |
| Pinned の競合 | `IllegalArgumentException`（例外） | `SpawnError::DispatcherAlreadyOwned`（回復可能エラー） |
| Balancing の分配 | 登録失敗時に team を走査（work donating） | dispatch のたびに primary + team を候補リストとして返す |
| executor の選択 | 設定文字列 + FQCN リフレクション | `ExecutorFactory` port の明示注入 |
| throughput 既定 5 / deadline 無効 / shutdown 1s / shutdown 3 状態 FSM | 同一（fraktor が parity 対象） | 同一 |

fraktor RFC 0003 の OQ-DISP-1（InlineExecutor が既定）に対し、Pekko の既定は fork-join プール（並行実行）である。既定構成の並行性は両者で異なる。

## 4. 参照

- fraktor 側 RFC 0003、`Dispatcher.scala:71-89`（dispatch）、`Mailbox.scala:228-238`（run/再登録）、`BatchingExecutor.scala`
