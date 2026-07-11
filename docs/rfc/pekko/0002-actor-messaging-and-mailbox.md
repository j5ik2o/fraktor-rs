# RFC pekko-0002: メッセージングと mailbox（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala`, `dispatch/sysmsg/`, `dispatch/Mailboxes.scala`, `util/StablePriorityQueue.scala`, `actor/src/main/resources/reference.conf` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

## 1. 概要

Pekko の mailbox は `_status`（`Int` 1 語、VarHandle CAS）で「Closed / Scheduled / suspend カウント」を管理し、user メッセージは pluggable な `MessageQueue`、システムメッセージは侵入型連結リストで運ぶ。

## 2. 規範仕様

### 2.1 status ビットフィールド（`Mailbox.scala`）

- **PMB-1.** 定数: `Open = 0` / `Closed = 1`（bit0）/ `Scheduled = 2`（bit1）/ `suspendUnit = 4`（bit2 以降が suspend カウント）。すべての遷移は `@tailrec` CAS ループである。
- **PMB-2.** 遷移の意味:
  - `setAsScheduled` — 下位 2 bit が `Open` のときのみ成功（**suspend カウントは無視**。suspend 中でもスケジュール可能で、その run はシステムメッセージのみを処理する）
  - `setAsIdle` — `Scheduled` ビットのみクリア
  - `becomeClosed` — 現在の状態を破棄して無条件に `Closed` へ。Closed は吸収状態（suspend / resume は Closed を上書きしない）
  - `suspend()` — カウント +1。戻り値は「初回 suspend だったか」
  - `resume()` — カウント −1（0 で飽和）。戻り値は「0 に戻った（再開すべき）か」
- **PMB-3.** `shouldProcessMessage` は `(status & ~Scheduled) == 0`、すなわち「Closed でなく suspend カウント 0」のときのみ user メッセージを処理してよい（MUST）。

### 2.2 実行ループ

- **PMB-4.** `run()` は「未 Closed なら システム全処理 → user 処理」の順で実行し、finally で必ず `setAsIdle()` の後 **自分自身を `registerForExecution(hint=false,false)` で再登録**する。再登録の可否は実際のキュー状態（`hasMessages` / `hasSystemMessages`）で判定される。
- **PMB-5.** `processMailbox` は `max(throughput, 1)` 件を上限に user メッセージを処理し、**1 件処理するごとに `processAllSystemMessages()` を挟む**（`Mailbox.scala:274`。「Never ever execute normal message when system message present」）。`throughput-deadline-time` が正の場合は `System.nanoTime` ベースの期限超過でループを打ち切る。
- **PMB-6.** `processAllSystemMessages` は suspend に関係なく実行され、drain 後に新着があれば再 drain する。処理中に mailbox が Closed になった場合、残りのシステムメッセージは deadLetterMailbox へ転送される。
- **PMB-7.** suspension は user の dequeue のみを阻止し、enqueue には一切影響しない（`enqueue` は status を参照しない）。

### 2.3 MessageQueue 実装（12 種）

| MailboxType | 満杯時 | 順序保証 |
|-------------|--------|---------|
| `UnboundedMailbox`（既定） | — | FIFO |
| `SingleConsumerOnlyUnboundedMailbox` | — | FIFO（MPSC 専用、BalancingPool 不可） |
| `NonBlockingBoundedMailbox` | 即時 deadLetters（新規を破棄） | FIFO（MPSC 専用） |
| `BoundedMailbox` | `pushTimeOut` だけ**ブロッキング** offer、超過で deadLetters | FIFO |
| `UnboundedPriorityMailbox` / `BoundedPriorityMailbox` | （Bounded は pushTimeOut → deadLetters） | 優先度順。同一優先度の FIFO 保証なし |
| `UnboundedStablePriorityMailbox` / `BoundedStablePriorityMailbox` | 同上 | 優先度順 + 同一優先度 FIFO（`WrappedElement(seqNum)` タイブレーク） |
| `UnboundedDequeBasedMailbox` / `BoundedDequeBasedMailbox` | 同上（`enqueueFirst` も pushTimeOut 対応） | FIFO + 先頭挿入（Stash 用） |
| `UnboundedControlAwareMailbox` / `BoundedControlAwareMailbox` | 同上（容量は 2 キュー合計） | `ControlMessage` 優先、各キュー内 FIFO |

- **PMB-8.** 満杯時の挙動は一貫して「**送信側の新規メッセージを deadLetters へ送る**」であり、キュー内の既存メッセージを追い出す戦略や容量を伸長する戦略は存在しない（MUST NOT に相当する設計選択）。Bounded 系は `mailbox-push-timeout-time`（既定 10s）のブロッキングを伴う。
- **PMB-9.** 既定 mailbox は `pekko.actor.default-mailbox` = `UnboundedMailbox`（`mailbox-capacity = 1000` / `mailbox-push-timeout-time = 10s` は bounded 選択時に効く）。解決順序は「Props(deploy) 明示 → dispatcher 設定の mailbox-type → actor の `RequiresMessageQueue` → dispatcher の mailbox-requirement → 既定」の 5 段（`Mailboxes.getMailboxType`）。

### 2.4 システムメッセージキュー

- **PMB-10.** システムメッセージは `SystemMessage.next` をリンクに使う侵入型単方向リストで、enqueue は先頭追加（LIFO）、`systemDrain` で一括取得時に `.reverse` して FIFO を復元する。
- **PMB-11.** close 済み mailbox（`NoMessage` センチネル）への `systemEnqueue` は deadLetterMailbox へ転送される。
- **PMB-12.** `cleanUp()` は system → user の順で残留メッセージを deadLetterMailbox へ転送する。actor の `unregister` は「mailbox を deadLetterMailbox にスワップ → `becomeClosed()` → `cleanUp()`」の順で行われ、以後その参照への tell は DeadLetter として観測される。

## 3. 状態機械（status 語）

状態要素は `Closed`（bit0、吸収状態）/ `Scheduled`（bit1）/ suspend カウント（bit2 以降、4 刻み）の 3 つで、1 つの `Int` に合成される。遷移表:

| 遷移 | 事前条件 | 事後状態 |
|------|---------|---------|
| `setAsScheduled` | 下位 2 bit が `Open`（suspend カウントは不問） | `Scheduled` ビット設定 |
| `setAsIdle` | — | `Scheduled` ビットのみクリア |
| `suspend()` | Closed でない | suspend カウント +1（戻り値: 初回か） |
| `resume()` | Closed でない | suspend カウント −1（0 で飽和。戻り値: 0 に戻ったか） |
| `becomeClosed` | — | 無条件に `Closed`（以後の全遷移を吸収） |

## 4. 不変条件

- **INV-PMB-1**: user メッセージ 1 件の処理前に、システムキューは空である（PMB-5）。
- **INV-PMB-2**: suspension は dequeue のみを阻止し、enqueue を失敗させない（PMB-7）。
- **INV-PMB-3**: `Closed` は吸収状態であり、いかなる遷移でも Open / Scheduled へ戻らない（PMB-2）。
- **INV-PMB-4**: bounded mailbox のあふれで失われるのは常に**新規（送信側）メッセージ**であり、キュー内の既存メッセージは失われない（PMB-8）。
- **INV-PMB-5**: システムメッセージの観測順序は到着順である（LIFO 蓄積 + drain 時反転、PMB-10）。

## 5. 参照

- `Mailbox.scala`（status 定数: 46-55 行 / run: 228-238 行 / per-message drain: 274 行 / cleanUp: 338-352 行）
