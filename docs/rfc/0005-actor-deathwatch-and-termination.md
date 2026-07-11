# RFC 0005: DeathWatch と終了

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/actor/actor_cell_death_watch.rs`, `actor/watch_kind.rs`, `system/termination_state.rs`, `system/termination_future.rs`, `system/termination_signal.rs`, `system/coordinated_shutdown.rs`, `system/base.rs`, `system/guardian/`, `system/remote/` |
| 関連文書 | RFC 0004（finish_terminate との連動）, `CONTEXT.md`（DeathWatch / Coordinated Shutdown） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

DeathWatch (死亡監視)、Coordinated Shutdown (協調シャットダウン)。`CONTEXT.md` の定義どおり、DeathWatch は「他 actor の終了を観測する契約」であり、Lifecycle Event の発行や child registry の所有とは別の境界である。

## 2. 概要

watch は「watcher が対象の終了時に `DeathWatchNotification` を受け取る」契約である。この通知は利用者向けの観測（`Terminated`）と、supervision / 終了の内部駆動（RFC 0004 の deferred `finish_recreate` / `finish_terminate`）の両方を支える。システム全体の終了は guardian 階層の DeathWatch 連鎖（user → system → root）として実装され、`TerminationState` が終了フラグと waker を管理する。

## 3. 規範仕様

### 3.1 watch / unwatch（宣言された挙動）

- **DW-1.** watch エントリは `WatchKind`（`User` / `Supervision`）でタグ付けされなければならない（MUST）。利用者の `unwatch` が、`finish_recreate` / `finish_terminate` を駆動する内部 supervision watch を誤って外すことを防ぐためである（`watch_kind.rs` に宣言）。
- **DW-2.** 既に終了している（cell は残っているが `is_terminated()` が真）対象への watch は、即座に watcher へ `DeathWatchNotification` を送る（`handle_watch` の notify_immediately）。
- **DW-3.** cell がレジストリから既に消えている対象への watch は、`SendError::Closed` を検知した watcher 側が**自分自身へ** `DeathWatchNotification(target)` をベストエフォート送信する（`ActorContext::watch`）。したがって「存在しない相手を watch しても通知は届く」。
- **DW-4.** 終了時の通知配送は `notify_watchers_on_stop` が watcher 集合を `mem::take` して全員に送る。受信側 `handle_death_watch_notification` の処理順序: watching から除去 → `terminated_queued` へ dedup 付き記録 → `ChildrenContainer::remove_child_and_get_state_change` → user watch があれば `watch_with` メッセージ配送または `on_terminated` 呼び出し → 状態変化が `Recreation` なら `finish_recreate`、`Termination` なら `finish_terminate` を駆動。
- **DW-5.** `watch_with`（カスタム終了メッセージ）の重複登録は契約違反であり、`ActorContext::watch_with` が事前チェックする。違反は debug ビルドで panic する。

### 3.2 remote への委譲（宣言された挙動）

- **DW-6.** 対象 pid がローカル cell でも temp actor でもない場合、`Watch` / `Unwatch` / `DeathWatchNotification` は `RemoteWatchHook` port へ委譲される（`system_state_shared.rs`）。`handle_watch` が「未消費」を返した場合は、その場で watcher へ `DeathWatchNotification` を合成送信する（ローカルにもリモートにも存在しない対象の即時死亡通知）。既定実装は `NoopRemoteWatchHook`。

### 3.3 システム終了（宣言された挙動）

- **DW-7.** `TerminationState` の契約:
  - `begin_termination()` は CAS（swap）であり、最初の呼び出しのみ true を返す
  - `mark_terminated()` は冪等。初回のみ waker 集合をロック内で take し、**ロック外で** wake する（デッドロック回避）
  - `register_waker` はロック取得後に終了フラグを double-check して lost-wakeup を防ぐ（`TerminationFuture::poll` 側にも同じ double-check がある）
- **DW-8.** `ActorSystem::terminate()` のシーケンス: 終了済みなら no-op → `begin_termination` に勝ったら scheduler を shutdown し、root guardian へ `StopChild(user_guardian)` を送る（root / user が欠けている場合の縮退経路あり）。負けたら `ForceTerminateHooks` を system guardian へ送るのみ。
- **DW-9.** guardian 停止連鎖は DeathWatch で実装される（MUST）:
  1. user guardian が停止 → system guardian（pre_start で user を watch 済み）が `Terminated` を観測
  2. system guardian は termination hook（`SystemGuardianProtocol::RegisterTerminationHook` で登録された外部コンポーネント）へ `TerminationHook` を配り、完了（または `ForceTerminateHooks`）後に自分を stop
  3. root guardian（pre_start で system を watch 済み）が `Terminated` を観測して `state.mark_terminated()` を呼ぶ
  root guardian 自身は停止メッセージを受けず、観測者としてのみ振る舞う。
- **DW-10.** root guardian の supervisor strategy は「常に Stop、restart 0 回」であり、escalate の最終防壁である（`root_guardian_actor.rs`）。
- **DW-11.** `finish_terminate` 側にも guardian 縮退時の安全網がある: User / System guardian が通常経路外で終了した場合、Root が既に居なければ `mark_terminated()` を直接呼ぶ（`actor_cell_lifecycle.rs`）。
- **DW-12.** 終了の観測 API は `when_terminated() -> TerminationSignal`（clone 可能、`.await` 可能、`wait_blocking(blocker)` 可能）と `run_until_terminated(blocker)`。

### 3.4 Coordinated Shutdown（宣言された挙動）

- **DW-13.** フェーズは 9 個の文字列定数で、既定依存グラフに従い Kahn のトポロジカル順で実行される: `before-service-unbind` → `service-unbind` → `service-requests-done` → `service-stop` → `before-cluster-shutdown` → `cluster-leave` → `cluster-shutdown` → `before-actor-system-terminate` → `actor-system-terminate`。
- **DW-14.** フェーズ内タスクは並行実行され、フェーズごとにタイムアウト（既定 5 秒）を持つ。タイムアウト時、`recover = false` のフェーズであれば全体を中断する。`run()` は冪等（2 回目以降は進行中の実行の完了を待つ）。
- **DW-15.** タスク登録は `add_task(phase, name, task)`。実行開始後の追加・未知フェーズ・空名はエラーになる。
- **DW-16.** `CoordinatedShutdown` は Extension として提供され、**`ActorSystem::terminate()` からは呼ばれない**。Pekko のような「terminate が Coordinated Shutdown を経由する」自動連携は現状存在しない → OQ-DW-1。

## 4. 状態機械

- **TerminationState**: `idle → terminating → terminated`（2 本の AtomicBool + waker 集合。begin は CAS、mark は冪等）。
- **guardian 生存フラグ**: Root / System / User の 3 本の AtomicBool（`mark_guardian_stopped` / `guardian_alive`）。
- watch 関係は状態機械というより二部関係（watchers / watching の対集合）であり、INV-DW-1〜3 の対象。

## 5. 不変条件

- **INV-DW-1**: watch した対象が終了した場合、watcher は必ずちょうど 1 回 `DeathWatchNotification` を観測する（生存対象 → DW-4、終了済み cell → DW-2、cell 不在 → DW-3、remote → DW-6 の全経路で成立。`terminated_queued` の dedup が重複を防ぐ）。
- **INV-DW-2**: 利用者の `unwatch` は supervision watch を解除しない（DW-1）。
- **INV-DW-3**: watcher / watching の対は片側だけ残らない（spawn 時: RFC 0004 SUP-3、終了時: `mem::take` による一括配送）。
- **INV-DW-4**: `mark_terminated` 後に `register_waker` された waker が起こされないことはない（double-check により成立）。
- **INV-DW-5**: システム終了フラグが立つのは root guardian の観測（DW-9）または縮退安全網（DW-11）のちょうど 1 経路であり、`mark_terminated` の冪等性により多重実行は無害である。

## 6. 機械的な問いへの回答

- **空/未設定のとき?** — watch 対象が不在でも通知は届く（DW-3）。termination hook が 1 つもなければ system guardian は即 stop。
- **同時に 2 つ来たら?** — `terminate()` の同時呼び出しは `begin_termination` の CAS で一方に絞られ、他方は hooks の強制のみ行う。watch と終了の競合は DW-2 / DW-3 / put された通知の dedup で吸収される。
- **このループは何で止まる?** — guardian 連鎖は 3 段で有限。Coordinated Shutdown はフェーズ DAG + タイムアウトで停止性を持つ。
- **2 つのシステムが合意しているか?** — 「終了したか」の真実は `TerminationState` のみが持ち、guardian 生存フラグは観測補助である。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-DW-1 | `terminate()` は Coordinated Shutdown を経由しない（DW-16）。Pekko では CoordinatedShutdown が terminate をフックする | 連携は将来実装か、明示的に `run()` を呼ぶ運用が正か | graceful shutdown の既定動作の互換性。cluster-leave 等のフェーズが terminate 時に自動実行されない |
| OQ-DW-2 | `ActorSystemBuildError::MissingTickDriver` variant が定義されているがどこからも構築されず、実際のガードは文字列ベースの `SystemBuildError("tick driver is required")` | variant を配線するか削除するか | エラー分類の一貫性（RFC 0006 とも関連） |

形式化候補（Lean）: INV-DW-1（exactly-once 通知）は「watch 登録・対象終了・cell 除去・remote 委譲」のインターリーブに対する定理として最重要のモデル化対象。guardian 3 段連鎖 + `TerminationState` は小さな状態空間で全数検査が可能であり、INV-DW-5 の唯一性・DW-7 の lost-wakeup 不在を検証できる。

## 8. 参照

- Pekko: `DeathWatch.scala` / `RootGuardian` / `CoordinatedShutdown`
- RFC 0004（finish_recreate / finish_terminate の駆動）、RFC 0006（scheduler shutdown）、RFC 0009（RemoteWatchHook port）
