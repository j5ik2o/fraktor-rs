# RFC pekko-0009: 実行環境接続（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/{AbstractDispatcher,Dispatchers}.scala`, `actor/ActorSystem.scala`, `actor/src/main/resources/reference.conf`, `references/pekko/docs/src/main/paradox/typed/dispatchers.md` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0009](../0009-actor-port-adaptor-contract.md) |
| 最終照合日 | 2026-07-11 |

本 RFC は fraktor の port-adaptor 契約に**対応する Pekko 側の仕組み**を記述する。Pekko には port trait / adaptor クレートに相当する抽象境界が存在しないため、対比の軸は「実行環境の何を・どうやって差し替えるか」である。

## 1. 規範仕様

- **PPORT-1.** 実行環境の差し替えはすべて **HOCON 設定 + FQCN リフレクション**で行う。port trait の階層は存在しない。
  - executor: `executor = "default-executor" | "fork-join-executor" | "thread-pool-executor" | "affinity-pool-executor" | "virtual-thread-executor" | <FQCN of ExecutorServiceConfigurator>`
  - dispatcher: `type = "Dispatcher" | "PinnedDispatcher" | <FQCN of MessageDispatcherConfigurator>`
  - scheduler: `pekko.scheduler.implementation = <FQCN of Scheduler>`
- **PPORT-2.** `default-executor` は外部から `ExecutionContext` が注入されていればそれを使い、なければ `fallback = fork-join-executor` に倒れる。既定構成の実行は fork-join プール（並行）である。
- **PPORT-3.** カスタム executor は `ExecutorServiceConfigurator(config, prerequisites)` を継承し、`dynamicAccess.createInstanceFor`（リフレクション）で生成される。dispatcher も同様に `MessageDispatcherConfigurator` のプラグイン + エイリアス解決（`Dispatchers.lookupConfigurator`、深さ上限つき）で構成される。
- **PPORT-4.** 時間は ActorSystem 内蔵の scheduler（既定 `LightArrayRevolverScheduler`、専用スレッド）から供給される。**外部 tick 供給という概念は存在しない**（pekko-0006 PSCH-1）。
- **PPORT-5.** ブロッキング処理の隔離は「専用 dispatcher を設定で切る」ことが公式ガイダンスである（bulk-heading）。既定でも `default-blocking-io-dispatcher`（thread-pool-executor, fixed-pool-size = 16, throughput = 1）が内部 I/O 用に用意されている。
- **PPORT-6.** panic（JVM では例外）の捕捉は invoke 経路の try/catch に組み込まれており、差し替え可能な guard 抽象はない（すべての Throwable が supervision に入る。VM エラー等の fatal は JVM 既定に従う）。

## 2. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| 抽象化の方式 | 設定文字列 + FQCN リフレクション（実行時解決） | port trait + adaptor クレート（コンパイル時解決、lint で境界強制） |
| 差し替え可能な点 | executor / dispatcher / scheduler 実装 / mailbox type | TickDriver / Executor / Blocker / Clock / InvokeGuard / EventStreamSubscriber / LoggerWriter / MailboxFactory / ActorRefProvider / Remote hooks |
| 時間 | 内蔵スケジューラスレッド（実時間前提） | 外部 tick 供給（no_std で実時間を持たないため） |
| ブロッキング隔離 | 設定ガイダンス（専用 dispatcher） | `tokio_actor_system_config` が default / blocking dispatcher を分離構成（同趣旨をコードで提供） |
| 例外/panic 捕捉 | ランタイム組込み（差し替え不可） | `InvokeGuard` port（既定素通し、std は catch_unwind） |
| 検証 | 実行時（設定エラーは起動時例外） | コンパイル時 + dylint |

fraktor の設計は「Pekko が設定とリフレクションで実現している可変点を、型と依存方向で表現し直したもの」と要約できる。fraktor RFC 0009 OQ-PORT-1（embassy の Blocker/Clock 欠如）に対応する問題は Pekko には存在しない（JVM 固定のため）。

## 3. 参照

- fraktor 側 RFC 0009、`lints/port-adaptor-boundary-lint/SPEC.md`
- `AbstractDispatcher.scala:353-410`（configureExecutor）、`Dispatchers.scala:159-213`、`reference.conf:359-643`、`references/pekko/docs/src/main/paradox/typed/dispatchers.md`（blocking ガイダンス）
