# RFC pekko-0009: 実行環境接続（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/{AbstractDispatcher,Dispatchers}.scala`, `actor/ActorSystem.scala`, `actor/src/main/resources/reference.conf`, `references/pekko/docs/src/main/paradox/typed/dispatchers.md` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

本 RFC は「実行環境（executor / dispatcher / scheduler / mailbox 実装）の何を・どうやって差し替えられるか」を記述する。

## 1. 規範仕様

- **PPORT-1.** 実行環境の差し替えはすべて **HOCON 設定 + FQCN リフレクション**で行う。
  - executor: `executor = "default-executor" | "fork-join-executor" | "thread-pool-executor" | "affinity-pool-executor" | "virtual-thread-executor" | <FQCN of ExecutorServiceConfigurator>`
  - dispatcher: `type = "Dispatcher" | "PinnedDispatcher" | <FQCN of MessageDispatcherConfigurator>`
  - scheduler: `pekko.scheduler.implementation = <FQCN of Scheduler>`
- **PPORT-2.** `default-executor` は外部から `ExecutionContext` が注入されていればそれを使い、なければ `fallback = fork-join-executor` に倒れる。既定構成の実行は fork-join プール（並行）である。
- **PPORT-3.** カスタム executor は `ExecutorServiceConfigurator(config, prerequisites)` を継承し、`dynamicAccess.createInstanceFor`（リフレクション）で生成される。dispatcher も同様に `MessageDispatcherConfigurator` のプラグイン + エイリアス解決（`Dispatchers.lookupConfigurator`、深さ上限つき）で構成される。
- **PPORT-4.** 時間は ActorSystem 内蔵の scheduler（既定 `LightArrayRevolverScheduler`、専用スレッド）から供給される（pekko-0006 PSCH-1）。
- **PPORT-5.** ブロッキング処理の隔離は「専用 dispatcher を設定で切る」ことが公式ガイダンスである（bulk-heading）。既定でも `default-blocking-io-dispatcher`（thread-pool-executor, fixed-pool-size = 16, throughput = 1）が内部 I/O 用に用意されている。
- **PPORT-6.** 例外の捕捉は invoke 経路の try/catch にランタイム組込みであり、差し替え点ではない（すべての Throwable が supervision に入る。VM エラー等の fatal は JVM 既定に従う）。

### 1.1 executor 構築の要点

- **PPORT-7.** スレッド数はすべて共通式 `scaledPoolSize(floor, factor, ceiling) = min(max(ceil(cores × factor), floor), ceiling)` で決まる。既定値: fork-join は `parallelism-min = 8` / `factor = 1.0` / `max = 64`（別途 `maximum-pool-size = 32767`）、thread-pool は `core-pool-size-min = 8` / `factor = 3.0` / `max = 64`（`fixed-pool-size` 指定時は core = max 固定）。
- **PPORT-8.** fork-join の `task-peeking-mode` は `"FIFO"`（asyncMode = true、キュー的）と `"LIFO"`（スタック的）の 2 値で、それ以外は `IllegalArgumentException`。thread-pool の `task-queue-type` は `"linked"`（既定、無制限）/ `"array"`（`task-queue-size` 有効時）。
- **PPORT-9.** 仮想スレッド対応（`virtualize = on` / `virtual-thread-executor`）は **JDK 21 以上でのみ有効**であり、未満の環境では通常 executor に倒れる。実装は `MethodHandles` リフレクション経由で `newThreadPerTaskExecutor` / `Thread.ofVirtual` を呼び、`VirtualizedExecutorService` がキャリアプールへの shutdown カスケードと負荷判定（`atFullThrottle`）を仲介する。
- **PPORT-10.** 拒否時の挙動は `SaneRejectedExecutionHandler`: shutdown 済みなら `RejectedExecutionException` を投げ（`CallerRunsPolicy` のような無言破棄をしない）、それ以外は呼び出しスレッドで実行する。
- **PPORT-11.** `CachingConfig` が actor 生成・mailbox 選択のホットパスで HOCON のパス存在判定と文字列取得のみをキャッシュする（数値系 getter は素通し。設定読み取りが実行時コストである JVM 構成固有の緩和策）。

## 2. 参照

- `AbstractDispatcher.scala:353-410`（configureExecutor）、`Dispatchers.scala:159-213`、`reference.conf:359-643`、`references/pekko/docs/src/main/paradox/typed/dispatchers.md`（blocking ガイダンス）
- `ThreadPoolBuilder.scala:51-52, 316-321`（scaledPoolSize / SaneRejectedExecutionHandler）、`ForkJoinExecutorConfigurator.scala:73-149`、`VirtualThreadSupport.scala:37-117`、`VirtualizedExecutorService.scala:32-83`、`CachingConfig.scala:43-134`、`reference.conf:464-549`（executor 既定値）
