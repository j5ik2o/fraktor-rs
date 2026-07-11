# RFC pekko-0006: スケジューラと時間（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/LightArrayRevolverScheduler.scala`, `actor/Scheduler.scala`, `actor/dungeon/ReceiveTimeout.scala`, `actor/Timers.scala`, `actor/FSM.scala`, `actor/src/main/resources/reference.conf` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

## 1. 規範仕様

### 1.1 LightArrayRevolverScheduler

- **PSCH-1.** 既定値: `tick-duration = 10ms`（最小: Windows 10ms / その他 1ms。未達は既定で `IllegalArgumentException`）、`ticks-per-wheel = 512`（2 の冪必須）、`shutdown-timeout = 5s`。scheduler は**専用スレッド**をライブラリ内部で起動して回る。
- **PSCH-2.** 遅延は tick の倍数へ**切り上げ**られる（最大 1 tick 遅れて実行されうる）。最大遅延は `tick-duration × Int.MaxValue`。
- **PSCH-3.** `scheduleOnce` の遅延が 0 以下の場合、ホイールを経由せず**即時に `ec.execute`** され、キャンセル不能（`NotCancellable`）となる。周期スケジュールの period ≤ 0 は `IllegalArgumentException`。
- **PSCH-4.** `scheduleWithFixedDelay` は「完了から delay」（遅延は蓄積）、`scheduleAtFixedRate` は「レート補償」（停止後にバースト実行されうる。fixedDelay がしばしば推奨、と scaladoc に明記）。
- **PSCH-5.** cancel は task スロットの CAS（`CancelledTask` への置換）であり、実行（`ExecutedTask` への置換）と同一 CAS を奪い合うため、二重実行・実行とキャンセルのレースは発生しない。
- **PSCH-6.** タイマースレッドはユーザーコードを実行せず、期限が来た task を紐づく `ExecutionContext`（通常は dispatcher）へ投入するのみ。actor 宛スケジュールの Runnable は `receiver ! message` を行う（mailbox 経由の配送）。
- **PSCH-7.** scheduler 停止時、残タスクのうち `TaskRunOnClose` 実装のみ実行し、他は破棄する。

### 1.2 Receive Timeout

- **PSCH-8.** タイムアウト発火時は**専用メッセージ `ReceiveTimeout`（case object）**が self へ送られる。
- **PSCH-9.** `NotInfluenceReceiveTimeout` マーカーを実装するメッセージはタイマーをリセットしない。ただしメッセージ処理中に `setReceiveTimeout` が呼ばれ設定が変わった場合は強制的に再スケジュールされる。実装は毎回 `scheduleOnce` を張り直す 1-shot 方式。

### 1.3 Timers（TimerScheduler）

- **PSCH-10.** 同一 key の start は旧タイマーをキャンセルして新 generation で置換する。タイマーメッセージは `TimerMsg(key, generation, owner)` ラッパーで運ばれ、受信時に「key 存在・owner 一致（restart 跨ぎ排除）・generation 一致」の 3 条件を満たす場合のみ実メッセージが配送される（stale 排除）。
- **PSCH-11.** `aroundPreRestart` と `aroundPostStop` の両方で `cancelAll()` が呼ばれる（restart / stop での全解除）。

### 1.4 classic FSM

- **PSCH-12.** state timeout は `TimeoutMarker(generation)` で運ばれ、**任意のメッセージ受信で generation が進む**ため、通常メッセージの到着が自然に古いタイムアウトを無効化する。`forMax(Duration.Inf)` は「stateTimeout 無効化」の特別マーカー。
- **PSCH-13.** `replying` の返信は遷移確定時に登録順（内部リストの reverse）で送信される。FSM 停止時は全 named timer と timeoutFuture をキャンセルする。

## 2. 不変条件

- **INV-PSCH-1**: 1 つの task が二重実行されることはなく、cancel 成功後に新規実行が開始されることもない（実行とキャンセルの単一 CAS、PSCH-5）。
- **INV-PSCH-2**: 実行時刻は要求遅延以上である（切り上げ、PSCH-2。ただし遅延 0 以下の即時実行を除く）。
- **INV-PSCH-3**: stale なタイマー/タイムアウトメッセージが現在状態に作用することはない（generation / owner 照合、PSCH-10 / PSCH-12）。

## 3. 参照

- `LightArrayRevolverScheduler.scala`（roundUp: 44-49 / cancel CAS: 359-420 / close: 171-193）、`dungeon/ReceiveTimeout.scala:40-76`、`Timers.scala:85-159`、`FSM.scala:860-919`
