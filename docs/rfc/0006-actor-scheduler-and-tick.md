# RFC 0006: スケジューラと tick

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/actor/scheduler/`, `actor/actor_cell_receive_timeout.rs`, `actor/actor_cell_timers.rs`, `actor/classic_timer_scheduler.rs`, `actor/fsm/` |
| 関連文書 | RFC 0009（TickDriver の adaptor 実装）, `CONTEXT.md`（Receive Timeout） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

Receive Timeout (受信タイムアウト)。ほかに実装語として tick（外部から供給される時間前進の単位）、resolution（1 tick が表す論理時間幅）を用いる。

## 2. 概要

カーネルは実時間を持たない。時間は `TickDriver` port（adaptor 実装）が `TickFeed` へ供給する tick 数としてのみ進み、`Scheduler`（timing wheel）が tick 到達で期限判定を行う。タイマーの発火は原則「対象 actor の mailbox へのメッセージ送信」であり、actor の逐次処理モデルを壊さない。

## 3. 規範仕様

### 3.1 Scheduler（宣言された挙動）

- **SCH-1.** コアエンジンは `Scheduler`（`scheduler/scheduler_core.rs`。「SchedulerCore」という型名は存在しない）。公開 API は `schedule_once` / `schedule_at_fixed_rate` / `schedule_with_fixed_delay` / `schedule_command` / `cancel` であり、いずれも `SchedulerHandle` を返す。
- **SCH-2.** `SchedulerHandle::cancel` は対象が `Scheduled` または `Executing` のときのみ成功する（`Pending` / 終端状態では false）。cancel はエントリを cancelled にするだけで、wheel からの即時除去は行わない（遅延除去）。
- **SCH-3.** 遅延→tick 変換は切り上げ（`div_ceil`、最小 1 tick）。`Duration::ZERO` の遅延は `SchedulerError::InvalidDelay` として拒否しなければならない（MUST）。過去期限のスケジュールは API 上表現できない。
- **SCH-4.** `SchedulerCommand` は 3 値であり、発火経路が異なる:
  - `SendMessage { receiver, message, sender }` — `receiver.try_tell(message)`。**mailbox 経由**で届き、失敗（対象停止済み等）は意図的に握りつぶすベストエフォート配送（実装コメントに宣言）
  - `RunRunnable { runnable }` — スケジューラ駆動スレッド上で**直接実行**（mailbox を経由しない）
  - `Noop`

### 3.2 TickDriver port（宣言された挙動）

- **SCH-5.** `TickDriver` の契約は `provision(self: Box<Self>, feed: TickFeedHandle, executor: SchedulerTickExecutor) -> Result<TickDriverProvision, TickDriverError>`。driver は feed へ tick を enqueue し、executor（の `drive_pending`）を何らかの手段で定期駆動する責務を負う。`TickDriverProvision` は resolution / kind / stopper（停止手段）を返す。
- **SCH-6.** `TickDriverError` は 7 値: `SpawnFailed` / `HandleUnavailable` / `UnsupportedEnvironment` / `DriftExceeded` / `DriverStopped` / `UnsupportedExecutor` / `InvalidResolution`。
- **SCH-7.** tick driver は `ActorSystemConfig` の必須要素であり、欠落時のシステム構築は失敗しなければならない（MUST。`system_state.rs` の `SpawnError::SystemBuildError("tick driver is required")`）。
- **SCH-8.** `TickDriverKind` は `Auto` / `Manual` / `Std` / `Tokio` / `Embassy`（non_exhaustive）。`Manual` の場合、runner API（テスト等から手動で tick を注入する口）が未設定なら自動で有効化される。
- **SCH-9.** provision 完了時に `EventStreamEvent::TickDriver(snapshot)` が発行され、駆動構成が観測可能である（RFC 0007）。
- **SCH-10.** 既定 resolution は 10ms（`SchedulerConfig::default()`）。

### 3.3 Receive Timeout（宣言された挙動）

- **SCH-11.** `set_receive_timeout(timeout, message)` は既存タイマーをキャンセルし、`schedule_once` で **呼び出し側が指定した任意の `AnyMessage` を自分自身へ self-send** するタイマーを張る。Pekko のような専用 `ReceiveTimeout` メッセージ型は存在しない（呼び出し側が識別可能なメッセージを渡す契約）。
- **SCH-12.** user メッセージの処理成功後、そのメッセージが `not_influence_receive_timeout` でない場合のみタイマーを再スケジュールする（リセット）。`AnyMessage::not_influence`（`NotInfluenceReceiveTimeout` trait 境界付き）で作られたメッセージはタイムアウトへ影響しない（Pekko `Actor.scala` の marker 対応）。
- **SCH-13.** `cancel_receive_timeout` で解除でき、actor 終了時（`finish_terminate`）には必ず解除される。

### 3.4 timers（宣言された挙動）

- **SCH-14.** actor 向けタイマーの公開面は `ActorContext::timers() -> ClassicTimerScheduler`（`start_single_timer` / `start_timer_with_fixed_delay` / `start_timer_at_fixed_rate` / `is_timer_active` / `cancel` / `cancel_all`）。
- **SCH-15.** 同一 key での再登録は既存タイマーを必ずキャンセルしてから行う（上書き）。登録時に失効エントリ（cancelled / completed）の掃除も行われる。
- **SCH-16.** actor の終了・再生成時にはすべてのタイマーハンドルが破棄される（正常終了と fault 経路の両方）。

### 3.5 classic FSM（宣言された挙動）

- **SCH-17.** `Fsm<State, Data>` は `when(state, handler)`（同一 state の二重登録は panic）/ `when_unhandled` / `on_transition` / `on_termination` / `start_with` / `initialize` で構成する。ハンドラは `FsmTransition`（`stay` / `goto` / `stop` / `unhandled`、`.using(data)` / `.for_max(timeout)` / `.replying(msg)` チェーン）を返す。
- **SCH-18.** state timeout は state ごとの設定値を持ち、FSM 全体で 1 つの timer key を使い回す。`timeout_generation` カウンタで stale なタイムアウトメッセージを破棄する。`for_max` は一時上書き（`Duration::ZERO` はキャンセル扱いに正規化）。
- **SCH-19.** `.replying` で積まれた返信は遷移適用後に登録順で送信され、送信失敗は伝播せず `record_send_error` に記録される（fire-and-forget）。
- **SCH-20.** named timer の `is_timer_active(name)` はスケジューラ側の生存確認ではなく、FSM ローカルの登録マップの有無のみを見る（暗黙の挙動）。

## 4. 状態機械

- **CancellableEntry**: `Pending → Scheduled → Executing → (Completed | Cancelled)` の CAS 状態機械。`Executing` 中の cancel も成功し、その周期ジョブは以後再スケジュールされない。`run_due` はコマンド実行直後と周期再登録前の 2 点で cancelled を再チェックし、cancel と発火の競合を吸収する。
- **時間の前進**: `current_tick` は `run_due(now)` で単調に進み、期限判定は timing wheel の `collect_expired` による。

## 5. 不変条件

- **INV-SCH-1**: kernel は tick の供給なしに時間を進めない（駆動が止まれば期限ジョブは発火せず pending のまま。エラーにも panic にもならない — 暗黙の挙動）。
- **INV-SCH-2**: cancel が true を返した後、そのジョブのコマンドが新たに実行開始されることはない（実行中だった 1 回は完了しうるが、再スケジュールは起きない）。
- **INV-SCH-3**: 期限は常に「登録時点の current_tick + 1 tick 以上先」である（SCH-3 の切り上げ + ZERO 拒否により成立）。
- **INV-SCH-4**: 1 つの actor に同一 key のタイマーが同時に 2 つ生存することはない（SCH-15）。
- **INV-SCH-5**: FSM の stale timeout（旧世代のタイムアウトメッセージ）が現在の状態に作用することはない（generation 照合により成立）。

## 6. 機械的な問いへの回答

- **空/未設定のとき?** — driver 未設定は構築エラー（SCH-7）。driver が沈黙した場合は静かな停止（INV-SCH-1）→ OQ-SCH-1。
- **エラー時の倒れ先は?** — スケジュール済みメッセージの配送失敗は握りつぶし（SCH-4、ベストエフォート宣言済み）。`SchedulerBackedDelayProvider` はスケジュール自体の失敗時に**即座に発火**へ倒す（fail-open。遅延が保証より短くなる方向）→ OQ-SCH-2。
- **境界はどっち向き?** — ZERO 遅延は拒否、変換は切り上げ（実遅延は指定以上）。
- **同時に 2 つ来たら?**（cancel と発火）— CAS 状態機械が調停（§4）。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-SCH-1 | driver が tick 供給を止めても何の観測イベントも出ず、ジョブは無期限 pending になる（INV-SCH-1） | driver 停止・drift の検出（`DriftExceeded` は定義済み）を運用面でどう観測させる想定か | タイマー依存機能（receive timeout / ask timeout / backoff）の静かな停止 |
| OQ-SCH-2 | `SchedulerBackedDelayProvider` はスケジュール失敗時に即発火する fail-open（遅延ゼロ化） | shutdown 中の意図的挙動か、呼び出し元がエラーを観測すべきか | 遅延を前提にした利用（retry backoff 等）が失敗時に密になりうる |
| OQ-SCH-3 | `ActorSystemBuildError::MissingTickDriver` が未配線（RFC 0005 OQ-DW-2 と同一事象） | 文字列エラーから variant への移行予定は | エラー分類の一貫性 |

形式化候補（Lean）: `CancellableEntry` の 5 状態 CAS 機械 ×「発火・cancel・周期再登録」のインターリーブ。INV-SCH-2 は「cancel 成功後の新規実行なし」という時相性質であり、モデル化の主対象。timing wheel の期限判定（切り上げ変換を含む）は境界値の等価類列挙に向く。

## 8. 参照

- RFC 0009（StdTickDriver / TokioTickDriver / TestTickDriver / EmbassyTickDriver の供給実装）
- RFC 0004（timers / receive timeout の actor 終了時解放）、RFC 0007（TickDriver スナップショットの観測）
