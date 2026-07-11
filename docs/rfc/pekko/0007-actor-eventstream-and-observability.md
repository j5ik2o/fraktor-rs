# RFC pekko-0007: EventStream と可観測性（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/event/{EventStream,EventBus,Logging,DeadLetterListener}.scala`, `actor/ActorRef.scala`（DeadLetter 型階層）, `actor/Actor.scala`（unhandled） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0007](../0007-actor-eventstream-and-observability.md) |
| 最終照合日 | 2026-07-11 |

## 1. 規範仕様

### 1.1 購読モデル

- **PEV-1.** 分類は **`Class[_]` の型階層**による: `subscribe(subscriber, channel: Class[_])` は `channel.isAssignableFrom(eventClass)` を満たすすべてのイベントを受け取る（SubchannelClassification。サブクラス購読が自動的に含まれる）。fraktor の固定 `ClassifierKey` 列挙とは異なるオープンな分類である。
- **PEV-2.** 購読者は `ActorRef` であり、配送は mailbox 経由の非同期 tell である（コールバック関数の同期呼び出しではない）。`EventStreamUnsubscriber` が購読者の終了を監視し、終了時に自動購読解除する。
- **PEV-3.** **リプレイバッファは存在しない**。購読時点より前のイベントは観測できない（pure pub-sub）。

### 1.2 Dead Letter と抑制

- **PEV-4.** 型階層は `AllDeadLetters` を頂点に `DeadLetter` / `SuppressedDeadLetter` / `Dropped` / `UnhandledMessage` が属する。
- **PEV-5.** **`DeadLetterSuppression`（空のマーカー trait）を実装するメッセージ**は、配送不能時に `DeadLetter` ではなく `SuppressedDeadLetter` に包まれて publish され、既定の `DeadLetterListener` はこれをログしない（MUST）。実装例: supervision 内部シグナル、TCP/IO 内部メッセージ、delivery の内部プロトコル、`PoisonPill` 等。
- **PEV-6.** dead letter のログは `log-dead-letters = 10`（件数上限）/ `log-dead-letters-during-shutdown = off` / `log-dead-letters-suspend-duration = 5 minutes` で制御される。
- **PEV-7.** `UnhandledMessage` は `Actor.unhandled` の既定実装が publish する（ただし `Terminated` は publish ではなく `DeathPactException` throw、pekko-0005 PDW-5）。

### 1.3 logging

- **PEV-8.** `EventStream` は `LoggingBus` を継承し、`loglevel` に応じて logger（`pekko.loggers` の FQCN で指定される actor、既定 `Logging$DefaultLogger`）の購読を動的に付け替える。起動初期は `StandardOutLogger`（`stdout-loglevel`、既定 WARNING）が仮 logger を務め、システム準備後に交代する。
- **PEV-9.** `LoggingAdapter` の実運用実装は `BusLogging`（EventStream への publish のみ）。`NoLogging` は no-op。

## 2. 不変条件

- **INV-PEV-1**: `DeadLetterSuppression` 実装メッセージが既定 listener の dead letter ログに現れることはない（PEV-5）。
- **INV-PEV-2**: 購読者の終了後にイベントが配送され続けることはない（EventStreamUnsubscriber による自動解除）。
- **INV-PEV-3**: publish はイベントのクラスに対して `isAssignableFrom` が成立する購読者すべてに（mailbox 経由で）届く（PEV-1）。

## 3. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| 分類 | `Class[_]` 型階層（オープン、サブタイプ包含） | 固定 `ClassifierKey`（15 値、`All` 以外は完全一致） |
| 購読者 | ActorRef（mailbox 経由・非同期） | コールバック trait（同期呼び出し、panic は伝播）。ActorRef 配送は `ActorRefEventStreamSubscriber` としてオプション |
| リプレイ | なし | あり（既定 256 件、subscribe 時に同期リプレイ） |
| Dead Letter 抑制 | `DeadLetterSuppression` marker + `SuppressedDeadLetter` + listener のログ抑制（実装済み） | reason タグのみで自動抑制なし（fraktor RFC 0007 OQ-EV-2 の裏付け——parity を取るなら marker と listener 側の抑制が必要） |
| UnhandledMessage の発行元 | untyped の `Actor.unhandled` | typed 層のみ（untyped kernel は型定義のみ） |
| logger の構成 | FQCN 設定 + 起動時交代プロトコル | `LoggerWriter` port + `LoggerSubscriber` の二段フィルタ |

## 4. 参照

- fraktor 側 RFC 0007
- `EventStream.scala:38-80`、`EventBus.scala:136-190`（SubchannelClassification）、`ActorRef.scala:551-696`（DeadLetter 階層と SuppressedDeadLetter 変換）、`DeadLetterListener.scala:154-159`、`reference.conf:17-68`
