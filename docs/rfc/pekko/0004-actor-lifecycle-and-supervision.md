# RFC pekko-0004: ライフサイクルと supervision（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorCell.scala`, `actor/dungeon/{FaultHandling,Children,ChildrenContainer}.scala`, `actor/FaultHandling.scala`, `actor/Actor.scala`, `dispatch/sysmsg/SystemMessage.scala` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0004](../0004-actor-lifecycle-and-supervision.md) |
| 最終照合日 | 2026-07-11 |

## 1. 規範仕様

### 1.1 SystemMessage プロトコル

- **PSUP-1.** `SystemMessage` は 10 variant: `Create` / `Recreate` / `Suspend` / `Resume` / `Terminate` / `Supervise` / `Watch` / `Unwatch` / `NoMessage` / `Failed`（+ `DeathWatchNotification`）。処理は `ActorCell.systemInvoke` が一括ディスパッチする。
- **PSUP-2.** システムメッセージにも **stash 機構**がある: cell の状態（Default / Suspended / SuspendedWaitForChildren）に応じ、`StashWhenFailed`（`Failed`）/ `StashWhenWaitingForChildren`（`Recreate` / `Suspend` / `Resume` / `Failed`）のマーカーを持つメッセージは保留され、状態が下がった時点で `unstashAll` により再処理される。子の全滅待ち中に届く再帰的な制御メッセージの順序を保証する仕組みである。

### 1.2 ChildrenContainer

- **PSUP-3.** 状態は `Empty` / `Normal` / `Terminating(reason)` / `Terminated` の 4 種で、遷移は不変コンテナの CAS スワップ（`@tailrec`）。`SuspendReason` は **4 variant**: `UserRequest` / `Recreation(cause)` / `Creation()` / `Termination`。`Creation()` は actor 生成（preStart 前後）失敗時の `faultCreate` → 子全滅後の `finishCreate` 遅延に使われる。
- **PSUP-4.** 最後の子が除去されたとき、reason が `Termination` なら `Terminated` へ、`Recreation` / `Creation` は `Normal` へ戻りつつ呼び出し元が `finishRecreate` / `finishCreate` を駆動する。

### 1.3 fault handling

- **PSUP-5.** `handleInvokeFailure` の手順: 自身を suspend → perpetrator を `FailedInfo`（`NoFailedInfo` / `FailedRef(ref)` / `FailedFatally`）に記録（処理中メッセージが子からの `Failed` ならその子、そうでなければ self）→ `suspendChildren(exceptFor = 失敗元の子)` で再帰 suspend（**失敗した子自身は既に suspend 済みのため除外**）→ 親へ `Failed(self, cause, uid)` を送る。
- **PSUP-6.** **Resume の伝播は perpetrator 限定の cause 付与**である: `resumeChildren(causedByFailure, perp)` は全子を resume するが、`causedByFailure` は `perp == child` の 1 体にのみ渡り、他の（巻き添え suspend された）子は `null` で resume される。cause 付き resume を受けた子だけが `clearFailed()` を行う。
- **PSUP-7.** `handleFailure` は `Failed.uid` と手元の `ChildRestartStats.uid` の一致を検証し、不一致（再作成前の古い子からの遅延 Failed）は無視する（MUST）。decider が escalate（false）を返すと `throw f.cause` により**自分自身の失敗**として親へ波及する。
- **PSUP-8.** 既定戦略は `OneForOneStrategy()(defaultDecider)` であり、**`maxNrOfRetries = -1`（無制限）/ `withinTimeRange = Duration.Inf`（時間窓なし）**。`defaultDecider` は `ActorInitializationException` / `ActorKilledException` / `DeathPactException` → Stop、その他 `Exception` → Restart、`Error` 系 → Escalate。
- **PSUP-9.** `AllForOneStrategy` の Restart は「全子が restart 許可を持つ場合のみ全員 restart（失敗元以外は suspend してから）、さもなければ全員 stop」。時間窓判定（`retriesInWindowOkay`）はウィンドウ開始からの経過時間でカウンタをリセットする方式。
- **PSUP-10.** `faultRecreate` は `aroundPreRestart`（失敗してもログのみ）→ `Recreation(cause)` を設定 → 生きた子がいれば全滅待ち、いなければ `finishRecreate`。`finishRecreate` は resume → `clearFailed` → 新インスタンス生成 → `aroundPostRestart` → **生存していた子を `child.restart(cause)` で再起動**（親が先、子が後の順序）。

### 1.4 ライフサイクルフックと停止

- **PSUP-11.** 既定実装: `preStart` / `postStop` は空。`preRestart` は「全子を unwatch + stop してから `postStop()`」。`postRestart` は `preStart()` を呼ぶ。すべて `around*` フック経由。
- **PSUP-12.** `PoisonPill` は **user キュー順で処理される** auto-received メッセージであり、順番が来たときに `self.stop()`（= system message `Terminate()`）を発火する。`context.stop(ref)` は Terminate を system message として即時送出するため、両者は「キュー内の位置」が異なる（MUST 区別）。
- **PSUP-13.** `Kill` は `ActorKilledException` を**メッセージ処理中に throw** することで通常の失敗経路（supervision）に載る。既定 decider は Stop に分類する。
- **PSUP-14.** `terminate()` の順序: receive timeout 解除 → `unwatchWatchedActors`（先に解除して DeadLetter(Terminated) を防ぐ）→ 全子へ stop → `Termination` reason 設定（子が生きていれば suspend + 待機、いなければ即 `finishTerminate`）。`finishTerminate` は `postStop` → dispatcher detach → 親へ `DeathWatchNotification(existenceConfirmed=true)` → その他 watcher へ通知 → フィールドクリア、の**厳密な順序**（コメントで明記）。

## 2. 不変条件

- **INV-PSUP-1**: 古い incarnation の子からの `Failed` が現世代の統計・戦略に作用することはない（UID 照合、PSUP-7）。
- **INV-PSUP-2**: 失敗処理中の再帰 suspend で、失敗元の子が二重に suspend されることはない（exceptFor、PSUP-5）。
- **INV-PSUP-3**: cause 付き resume を受けるのは失敗の perpetrator ちょうど 1 体である（PSUP-6）。
- **INV-PSUP-4**: 子が全滅するまで `finishRecreate` / `finishCreate` / `finishTerminate` は実行されない（Terminating reason + 待機、PSUP-3 / PSUP-10 / PSUP-14）。

## 3. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| SuspendReason | 4 variant（`Creation()` あり） | 3 variant（`Creation` は YAGNI で未実装と宣言） |
| system message の stash | あり（SuspendedWaitForChildren 等の状態で保留・再処理） | なし（相当機構は明示されていない） |
| Resume の伝播 | 全子 resume + cause は perpetrator のみ（clearFailed は cause 受領側のみ） | 全子へ無条件 Resume（cause 区別なし。fraktor RFC 0004 OQ-SUP-2） |
| suspendChildren | 失敗元の子を except（二重 suspend 防止） | 全子へ送信（except 機構は明示されていない） |
| 既定戦略 | retries 無制限・時間窓なし | `WithinWindow(10)` / 1 秒 |
| 既定 decider | 例外型ベース（ActorKilled/DeathPact/Init → Stop、他 → Restart） | エラー分類ベース（Recoverable → Restart / Fatal → Stop / Escalate → Escalate） |
| PoisonPill | user キュー順（auto-receive） | system queue 経由で `Stop` と同一ハンドラ（fraktor OQ-SUP-3 の裏付け） |
| Kill | 例外 throw → supervision（既定 Stop） | `ActorError::fatal("Kill")` を report_failure（経路は類似、既定の帰結も Stop 側） |
| Failed の UID 照合 | あり | 相当機構は明示されていない（要確認: fraktor 側 Open Question 候補） |
| preRestart 既定 | unwatch + stop 子 + postStop | stop_all_children + post_stop（unwatch は明示されていない） |

fraktor 側への還元: 上表の「stash 機構なし」「UID 照合なし」「suspend except なし」は fraktor RFC 0004 の Open Questions に追補する価値がある差分である。

## 4. 参照

- fraktor 側 RFC 0004
- `dungeon/FaultHandling.scala`（handleInvokeFailure: 215-245 / finishRecreate: 278-303 / terminate: 180-212 / finishTerminate: 247-276）、`dungeon/Children.scala:210-216`（resumeChildren）、`actor/FaultHandling.scala:216-230`（defaultDecider / defaultStrategy）
