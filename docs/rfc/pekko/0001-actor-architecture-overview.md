# RFC pekko-0001: Pekko actor アーキテクチャ概観

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0001](../0001-actor-architecture-overview.md) |
| 最終照合日 | 2026-07-11 |

## 1. 概要

Pekko の actor 実装は 2 モジュールで構成される（cluster / persistence / stream 系は本ミラーシリーズの対象外）。

| モジュール | 規模 | 主なパッケージ |
|-----------|------|---------------|
| `actor`（classic / untyped） | Scala 195 ファイル | `actor`（ActorCell / ActorSystem / FSM / CoordinatedShutdown / Scheduler）、`actor/dungeon`（ActorCell の分割実装）、`dispatch`（Mailbox / Dispatcher / sysmsg）、`event`（EventStream / logging）、`pattern`（ask / CircuitBreaker / retry）、`routing`、`serialization`、`io`、`util` |
| `actor-typed` | Scala 90 ファイル | `Behavior` / `BehaviorInterceptor` / `SupervisorStrategy`、`internal`（interpreter / adapter）、`receptionist`、`pubsub`、`delivery`、`eventstream`、`scaladsl` / `javadsl` |

## 2. 構造上の特徴（fraktor との対比の基礎）

- **P-1.** 実行環境は JVM に固定であり、no_std に相当する層分離は存在しない。スレッドプール・時刻・スケジューラはすべてライブラリ内蔵である。
- **P-2.** 実行環境の差し替えは port trait ではなく**設定（HOCON / `reference.conf`）+ FQCN プラグイン**で行う（`ExecutorServiceConfigurator` / `MailboxType` / dispatcher 設定。詳細は pekko-0009）。
- **P-3.** `ActorCell` は `actor/dungeon/` 配下の trait 群（`Children` / `DeathWatch` / `Dispatch` / `FaultHandling` / `ReceiveTimeout` / `TimerSchedulerImpl`）を mixin する形で責務分割される。fraktor の Actor Cell Facet（private sibling module、ADR 0002）はこの trait mixin 構造を「同一型 + private module」へ翻訳したものである。
- **P-4.** typed 層（`actor-typed`）は classic ランタイムの上に adapter（`internal/adapter`）で実装される。fraktor の typed 層（RFC 0008）と同じ位置づけ。
- **P-5.** システムメッセージは `dispatch/sysmsg` の `SystemMessage` 連結リスト（LIFO 受信 → 反転で FIFO 復元）で運ばれる。

## 3. モジュール対応表

| 領域 | Pekko | fraktor | ミラー RFC |
|------|-------|---------|-----------|
| mailbox | `dispatch/Mailbox.scala`（status ビットフィールド） | `dispatch/mailbox/`（`MailboxScheduleState`） | 0002 |
| dispatcher | `dispatch/{Dispatcher,PinnedDispatcher,BalancingDispatcher}.scala` | `dispatch/dispatcher/` | 0003 |
| lifecycle / supervision | `actor/dungeon/{FaultHandling,ChildrenContainer}.scala`, `actor/FaultHandling.scala` | `actor/actor_cell_*.rs`, `children_container.rs` | 0004 |
| DeathWatch / 終了 | `actor/dungeon/DeathWatch.scala`, `actor/CoordinatedShutdown.scala` | `actor_cell_death_watch.rs`, `system/` | 0005 |
| scheduler | `actor/LightArrayRevolverScheduler.scala` | `actor/scheduler/` + `TickDriver` port | 0006 |
| 観測 | `event/EventStream.scala`, `event/Logging.scala` | `event/` | 0007 |
| typed | `actor-typed` | `actor-core-typed` | 0008 |
| 実行環境接続 | 設定駆動（dispatcher / executor 設定） | port / adaptor 分離 | 0009 |
| routing / serialization / patterns | `routing/`, `serialization/`, `pattern/` | 同名モジュール | 0010 |

## 4. fraktor-rs との差分（構造レベル）

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| 環境分離 | JVM 固定、設定駆動 | no_std core + adaptor クレート、port trait 駆動（lint で機械的強制） |
| ActorCell 分割 | trait mixin（dungeon） | 同一型の private sibling module（ADR 0002） |
| 時間の供給 | 内蔵スケジューラスレッド | 外部 `TickDriver` が tick を供給（kernel は実時間を持たない） |
| 拡張実装の注入 | FQCN + リフレクション | trait object の明示的受け渡し（リフレクションなし） |

## 5. 参照

- fraktor 側 RFC 0001（本 RFC と同一テンプレート）
- `docs/gap-analysis/actor-gap-analysis.md`（API レベルの差分。本シリーズは意味論レベルを担当）
