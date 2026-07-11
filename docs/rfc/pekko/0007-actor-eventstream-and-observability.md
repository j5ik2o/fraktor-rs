# RFC pekko-0007: EventStream と可観測性（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/event/{EventStream,EventBus,Logging,DeadLetterListener}.scala`, `actor/ActorRef.scala`（DeadLetter 型階層）, `actor/Actor.scala`（unhandled） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

## 1. 規範仕様

### 1.1 購読モデル

- **PEV-1.** 分類は **`Class[_]` の型階層**による: `subscribe(subscriber, channel: Class[_])` は `channel.isAssignableFrom(eventClass)` を満たすすべてのイベントを受け取る（SubchannelClassification。サブクラス購読が自動的に含まれるオープンな分類である）。
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

### 1.4 EventBus 分類ファミリ

- **PEV-10.** `EventBus` の分類戦略は 4 ファミリ: `LookupClassification`（classifier の等価一致で `Index` を引く）/ `SubchannelClassification`（階層包含。EventStream が使う方式で、キャッシュミス時のみ `synchronized` で補充する `@volatile` キャッシュを持つ）/ `ScanningClassification`（`ConcurrentSkipListSet` を全走査して `matches` 判定）/ `ManagedActorClassification`（ActorRef → ActorRef の対応を `AtomicReference` CAS で管理）。
- **PEV-11.** `ManagedActorClassification` の購読者は `ActorClassificationUnsubscriber` が監視し、`Terminated` で自動購読解除される。`Register` / `Unregister` はシーケンス番号（`seq == nextSeq`）で厳密に順序制御され、順序が合わないものは stash → `unstashAll` で再処理される。
- **PEV-12.** `AddressTerminatedTopic` は `private[pekko]` の内部 extension であり、リモート参照の watcher が購読者として登録され、remote / cluster の death watch が publish する `AddressTerminated` を全購読者へ配送する。

### 1.5 debug 観測

- **PEV-13.** `LoggingReceive` は `pekko.actor.debug.receive` が on のときのみ `Receive` をラップし、`isDefinedAt` 評価時に「received handled/unhandled message」を Debug レベルで publish する（off なら元の `Receive` をそのまま返す no-op。MUST）。
- **PEV-14.** `LoggerMailbox` は logger actor 専用の mailbox であり、`cleanUp`（mailbox 差し替え / シャットダウン時）に残存する `LogEvent` を `StandardOutLogger` へ同期 flush してから破棄する（loglevel OFF ならスキップ）。シャットダウン中のログ喪失を防ぐ仕組みである。
- **PEV-15.** debug フラグは 7 種: `receive` / `autoreceive`（auto-received メッセージのログ）/ `lifecycle`（起動・監視開始）/ `fsm` / `event-stream` / `unhandled`（`UnhandledMessage` を購読する `UnhandledMessageForwarder` を起動してログへ転送）/ `router-misconfiguration`。

## 2. 不変条件

- **INV-PEV-1**: `DeadLetterSuppression` 実装メッセージが既定 listener の dead letter ログに現れることはない（PEV-5）。
- **INV-PEV-2**: 購読者の終了後にイベントが配送され続けることはない（EventStreamUnsubscriber による自動解除）。
- **INV-PEV-3**: publish はイベントのクラスに対して `isAssignableFrom` が成立する購読者すべてに（mailbox 経由で）届く（PEV-1）。

## 3. 参照

- `EventStream.scala:38-80`、`EventBus.scala:93-436`（4 分類ファミリ）、`ActorRef.scala:551-696`（DeadLetter 階層と SuppressedDeadLetter 変換）、`DeadLetterListener.scala:154-159`、`reference.conf:17-68`
- `ActorClassificationUnsubscriber.scala:29-100`、`AddressTerminatedTopic.scala:30-71`、`LoggingReceive.scala:41-105`、`LoggerMailbox.scala:45-75`、`ActorSystem.scala:452-458`（debug フラグ定義）
