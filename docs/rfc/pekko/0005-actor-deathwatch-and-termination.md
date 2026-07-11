# RFC pekko-0005: DeathWatch と終了（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/dungeon/DeathWatch.scala`, `actor/ActorRefProvider.scala`, `actor/ActorSystem.scala`, `actor/CoordinatedShutdown.scala`, `actor/src/main/resources/reference.conf` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0005](../0005-actor-deathwatch-and-termination.md) |
| 最終照合日 | 2026-07-11 |

## 1. 規範仕様

### 1.1 watch / unwatch

- **PDW-1.** `watch` と `watchWith` の混在は禁止であり、同一対象に対して異なる契約（通常 Terminated ⇔ カスタムメッセージ）で再 watch すると `IllegalStateException` になる（先に unwatch が必要。MUST）。
- **PDW-2.** `unwatch` は `terminatedQueued` のエントリも常に除去する（watch していなかった場合でも安全）。
- **PDW-3.** 対象終了時、watcher は `watchedActorTerminated` で `watching` から除去し、終了処理中（isTerminating）でなければ **`Terminated` を自分の user mailbox へ通常メッセージとして self-tell** する。`terminatedQueued` への記録と `receivedTerminated` の照合により同一 Terminated の二重配送を防ぐ。
- **PDW-4.** `Terminated` は `existenceConfirmed`（対象自身からの通知か）と `addressTerminated`（リモートノード unreachable 由来か）の 2 つの由来フラグを持つ。
- **PDW-5.** 受信した `Terminated` を receive が処理しなかった場合、`unhandled` の既定実装が `DeathPactException` を throw する。既定 decider はこれを Stop に分類する（watch したら Terminated を処理せよ、さもなくば死ぬ——death pact）。

### 1.2 guardian 階層とシステム終了

- **PDW-6.** 階層は `bubble-walker`（root の仮想親、`MinimalActorRef`）→ root guardian（`/`）→ system guardian（`/system`）+ user guardian（`/user`）。`init` 時に「system が user を watch」「root が system を watch」の連鎖が張られる。
- **PDW-7.** 終了連鎖: `finalTerminate()` が `guardian.stop()`（/user 停止）→ system guardian が `Terminated(user)` を観測し、登録済み termination hook 全員へ `TerminationHook` を送って完了を待ってから自分を停止 → root が `Terminated(system)` を観測して自分を停止 → bubble-walker が `DeathWatchNotification` を受けて `causeOfTermination` を完了させる。
- **PDW-8.** root guardian の戦略（`rootGuardianStrategy`）は Stop 系であり、`preRestart` を空実装にオーバーライドして「guardian は restart で子を失わない」ことを保証する。

### 1.3 CoordinatedShutdown と terminate の連携

- **PDW-9.** `ActorSystem.terminate()` は既定で **CoordinatedShutdown を実行する**: `run-by-actor-system-terminate = on`（既定）の場合、`CoordinatedShutdown(this).run(ActorSystemTerminateReason)` が走り、その**最終フェーズ `actor-system-terminate` に自動登録された `terminate-system` タスク**が `finalTerminate()` を呼ぶ二段構えである（MUST として設計されている）。
- **PDW-10.** `run-by-actor-system-terminate = on` かつ `terminate-actor-system = off` の組み合わせは不正であり、起動時に `ConfigurationException` になる。
- **PDW-11.** 既定値: `default-phase-timeout = 5s` / `terminate-actor-system = on` / `exit-jvm = off` / `run-by-jvm-shutdown-hook = on`（JVM シャットダウンフックからも `run(JvmExitReason)` される）/ `run-by-actor-system-terminate = on`。
- **PDW-12.** 既定フェーズは **12 個**: `before-service-unbind` → `service-unbind` → `service-requests-done` → `service-stop` → `before-cluster-shutdown` → `cluster-sharding-shutdown-region`(10s) → `cluster-leave` → `cluster-exiting`(10s) → `cluster-exiting-done` → `cluster-shutdown` → `before-actor-system-terminate` → `actor-system-terminate`(10s)。各フェーズは `timeout`（既定 5s）/ `recover`（既定 on）/ `enabled` / `depends-on` を持つ DAG。
- **PDW-13.** 終了の観測は `whenTerminated: Future[Terminated]`（registerOnTermination のコールバック完了後に完了）。組込みの同期ブロッキング API は存在せず、呼び出し側が `Await` 等で待つ。

## 2. 不変条件

- **INV-PDW-1**: 同一の watch 対象について `Terminated`（またはカスタムメッセージ）が二重に receive へ渡ることはない（terminatedQueued 照合、PDW-3）。
- **INV-PDW-2**: `Terminated` の未処理は必ず失敗（DeathPactException）に変換される（PDW-5）。
- **INV-PDW-3**: guardian 連鎖の停止順序は user → system → root で固定である（watch 連鎖により成立、PDW-6/7）。
- **INV-PDW-4**: 既定構成において、`terminate()` の呼び出しで CoordinatedShutdown の全フェーズが（recover 設定に従い）実行される（PDW-9）。

## 3. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| terminate と Coordinated Shutdown | **連携あり**（terminate → run → 最終フェーズが finalTerminate） | 連携なし（Extension として独立。fraktor RFC 0005 OQ-DW-1 の裏付け——Pekko parity を取るなら要実装） |
| フェーズ数 | 12（cluster-sharding / exiting / exiting-done を含む） | 9 |
| JVM/プロセスフック | `run-by-jvm-shutdown-hook = on` 既定 | なし（no_std kernel の責務外） |
| Terminated の配送 | user メッセージとして self-tell（receive で処理） | `DeathWatchNotification`（system message）+ `on_terminated` フック / `watch_with` |
| death pact | untyped でも既定（unhandled Terminated → DeathPactException） | untyped には無く、typed 層のみ（fraktor RFC 0008 TY-6） |
| Terminated のメタデータ | existenceConfirmed / addressTerminated | なし（Pid のみ） |
| watch/watchWith 混在 | `IllegalStateException` | 事前チェック + debug ビルド panic（同趣旨） |
| 同期待ち API | なし（Future のみ） | `TerminationSignal::wait_blocking(Blocker)` あり |

## 4. 参照

- fraktor 側 RFC 0005
- `ActorSystem.scala:1066-1088`（terminate / finalTerminate）、`CoordinatedShutdown.scala:238-296`（terminate-system タスク / JVM フック）、`ActorRefProvider.scala:295-374, 613-619`（guardian 連鎖）、`reference.conf:1230-1388`
