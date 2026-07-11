# RFC 0004: ライフサイクルと supervision

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/actor/actor_cell*.rs`, `actor/children_container.rs`, `actor/suspend_reason.rs`, `actor/failed_info.rs`, `actor/supervision/`, `actor/messaging/system_message.rs`, `actor/lifecycle/`, `system/base.rs`（spawn 経路） |
| 関連文書 | ADR 0002（Actor Cell Facet）, RFC 0002（system queue）, RFC 0005（DeathWatch との連動）, `CONTEXT.md`（Actor Cell / Actor Cell Facet） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

Actor Cell (アクターセル)、Actor Cell Facet (アクターセルファセット)。本 RFC では ADR 0002 に従い、facet は「同一型 `ActorCell` を責務単位の private sibling module に分割した実装構造」を指し、公開 trait ではない。

## 2. 概要

actor の生涯は「spawn（Create ハンドシェイク）→ メッセージ処理 → （失敗時）supervision による Resume / Restart / Stop / Escalate → 終了（finish_terminate）」で構成される。制御はすべて `SystemMessage`（system queue 経由、RFC 0002 MB-10 により user より優先）で駆動され、子の集合は `ChildrenContainer` の 4 状態機械が管理する。

## 3. 規範仕様

### 3.1 SystemMessage プロトコル（宣言された挙動）

`SystemMessage` は 13 variant であり、system queue 経由の処理先は次のとおり（`actor_cell_dispatch.rs` の `system_invoke`）:

| variant | 処理 |
|---------|------|
| `PoisonPill` / `Stop` | `handle_stop`（両者は**完全に同一のハンドラ**。意味的差異なし） |
| `Kill` | `handle_kill` — `ActorError::fatal("Kill")` を生成して**自分自身の失敗として** `report_failure` する（graceful stop ではなく supervision 経路） |
| `Create` | `handle_create` — `pre_start` 実行と `Started` 発行 |
| `Recreate(cause)` | `fault_recreate`（§3.4） |
| `Failure(payload)` | `handle_failure` — 子の失敗に対する supervisor 判定（§3.5） |
| `Suspend` / `Resume` | 子への再帰伝播（§3.4）。mailbox カウンタ自体は mailbox 層で処理済み |
| `Watch(pid)` / `Unwatch(pid)` / `DeathWatchNotification(pid)` | RFC 0005 |
| `StopChild(pid)` | `stop_child` — `ChildrenContainer::shall_die` + 子へ `Stop` |
| `PipeTask(id)` | pipe された Future の完了処理 |

- **SUP-1.** `PoisonPill` と `Kill` は user mailbox 経由（公開型の downcast）でも同じハンドラへ届く二重経路を持つ（`actor_cell_dispatch.rs` の `invoke`）。

### 3.2 spawn と Create ハンドシェイク（宣言された挙動）

- **SUP-2.** spawn の順序は「cell 登録 → 親への子登録 + supervision watch 登録 → `Create` 送信」でなければならない（MUST）。子が `pre_start` で失敗した時点で、親は既に子を `children_state` と watching 集合に持っている必要があり、これにより後続の `DeathWatchNotification` が `finish_recreate` / `finish_terminate` を確実に駆動する（`system/base.rs` の AC-H4 コメントとして宣言。TOCTOU-safe order）。
- **SUP-3.** supervision watch の登録時、親 cell が既に解放されていれば子側の登録もスキップする（片側だけの stale watcher を残さない。`system/base.rs`）。
- **SUP-4.** `Create` ハンドシェイク失敗時のロールバックは「supervision watch 解除 → 名前解放 → cell 除去 → 子登録解除」の順で行う（`rollback_spawn`）。

### 3.3 状態機械: `ChildrenContainer`（Pekko `ChildrenContainer.scala` 対応）

状態: `Empty` → `Normal { c }` → `Terminating { c, to_die, reason }` → `Terminated`。

| 遷移関数 | 遷移 |
|---------|------|
| `add_child` | `Empty → Normal`。`Normal` / `Terminating` は in-place 追加。`Terminated` では no-op |
| `shall_die` | `Normal → Terminating`（reason = `SuspendReason::UserRequest`）。`Terminating` は `to_die` へ追加のみ |
| `set_children_termination_reason` | `Terminating` の reason を `Recreation` / `Termination` へ書き換え（それ以外は no-op） |
| `remove_child_and_get_state_change` | `Terminating` で `to_die` が空になったとき、reason が `Termination` なら `Terminated` へ、それ以外は `Normal` / `Empty` へ戻り、観測された reason を返す |

- **SUP-5.** `remove_child_and_get_state_change` は状態変更と「遷移時に観測した reason」の返却を単一操作で行う。これは Pekko の CAS ループと等価にするための**意図的な CQS 違反**であり、コード上で人間許可済みと宣言されている（`children_container.rs`）。
- `SuspendReason` は 3 値: `UserRequest` / `Recreation(cause)` / `Termination`（Pekko の `Creation` は YAGNI として意図的に未実装）。

### 3.4 fault handling（宣言された挙動）

`FailedInfo` は 3 値: `None` / `Child(Pid)` / `Fatal`。`set_failed` は `Fatal` を上書きしない（Fatal 優先）。

失敗時の基本手順（`report_failure`、Pekko `handleInvokeFailure` 対応）:

1. `mailbox().suspend()`（自分の user 処理を停止。enqueue は継続 — RFC 0002 MB-12）
2. 未 failed なら `set_failed(self.pid)`
3. `suspend_children()` — 全子へ `Suspend` を送り、各子が再帰的に孫へ伝播
4. `system().report_failure(payload)` — 親があれば `SystemMessage::Failure` を親へ、なければ自分へ `Stop`

**Restart 判定後の再生成シーケンス**（`fault_recreate` → `finish_recreate`）:

1. `fault_recreate(cause)`: 既に `Fatal` なら no-op。`pre_restart` を **drive guard 配下**で実行（既定実装は全子 stop + `post_stop`。子 mailbox への同一スレッド再入を防ぐため — RFC 0003 DISP-13）
2. `deferred_recreate_cause` に cause を保存し、`ChildrenContainer` の reason を `Recreation(cause)` に設定
3. 生きている子がいれば**ここで中断**（deferred）。最後の子の `DeathWatchNotification` を受けた時点で `finish_recreate` が駆動される（RFC 0005）
4. `finish_recreate(cause)`: pipe/stash/timer/watch 資源を破棄 → `Stopped` 発行 → actor を factory から再生成 → `clear_failed` → `post_restart`（既定実装は `pre_start` へ委譲）→ 成功なら `mailbox().resume()` + `Restarted` 発行。`post_restart` 失敗時は resume 後に `set_failed_fatally` して再度 `report_failure`

- **SUP-6.** `pre_restart` / `post_restart` の既定実装は Pekko `Actor.scala` と同一であり、オーバーライドは既定処理を**完全に置換**する（カーネルはオーバーライド後に既定実装を再委譲しない）（`actor_lifecycle.rs` rustdoc に宣言）。
- **SUP-7.** `Resume` 受信時は `clear_failed` 後に `resume_children()` で全子へ無条件に `Resume` を伝播する。Pekko の perpetrator（失敗当事者）に限定した resume は未実装である（実装コメントに宣言）→ OQ-SUP-2。

### 3.5 supervision 判定（宣言された挙動）

- **SUP-8.** 子の `Failure(payload)` を受けた親は `SupervisorStrategy::decide`（decider）で `SupervisorDirective`（`Restart` / `Stop` / `Escalate` / `Resume`）を決める。適用範囲は `OneForOne` = 当該子のみ、`AllForOne` = 全子（`AllForOne` の Restart では兄弟を先に `Suspend` してから `Recreate` を送る）。
- **SUP-9.** `Restart` は `RestartStatistics::request_restart_permission`（回数 + 時間窓）を通らなければ `Stop` へ格上げされる（MUST）。この判定はチェックと適用を単一メソッドで行う意図的な CQS 例外である（TOCTOU ギャップ回避。`restart_statistics.rs` に宣言）。
- **SUP-10.** `within: Duration::ZERO` は「時間窓なし」の fraktor-rs センチネルであり、typed Pekko の `withinTimeRange = Duration.Zero` / classic Pekko の `withinTimeRangeOption = None` と等価である（`supervision/base.rs` に定義）。
- **SUP-11.** 既定戦略: `OneForOne`、`RestartLimit::WithinWindow(10)`、`within = 1 秒`、decider は `Recoverable → Restart` / `Fatal → Stop` / `Escalate → Escalate`。
- **SUP-12.** supervisor strategy の解決は常に `Actor::supervisor_strategy(ctx)` の動的呼び出しであり、`Props` から静的に注入する経路は存在しない（`SupervisorOptions` 型は定義されているが `Props` に配線されていない）→ OQ-SUP-1。
- backoff supervisor（`BackoffSupervisor` / `OnStop` / `OnFailure` の 2 モード、`min * 2^n` を `max` で飽和、jitter・auto_reset あり）は Props ファクトリとして提供される。

### 3.6 終了（宣言された挙動）

- **SUP-13.** `handle_stop` は生きている子がいれば「suspend + 全子へ `Stop` 送信」して**待つ**（`finish_terminate` は呼ばない）。子がいなければ即 `finish_terminate`。子の終了は `DeathWatchNotification` → `remove_child_and_get_state_change` → 状態変化が `Termination` になった時点で `finish_terminate` が駆動される。
- **SUP-14.** `finish_terminate` の順序: 子が空であることの確認 → `post_stop` → `Stopped` 発行 → stash/timer 破棄 → cell ローカル `mark_terminated` → **watcher への `DeathWatchNotification` 配送** → 親からの子登録解除 → 名前解放 + cell 除去 → guardian 判定（RFC 0005）。
- **SUP-15.** エラー分類は `ActorError` の 3 値（`Recoverable` / `Fatal` / `Escalate`）。panic の `ActorError` 化はカーネルの責務ではなく、`InvokeGuard` port（既定は素通しの `NoopInvokeGuard`）に対する adaptor 実装（std の `PanicInvokeGuard` が `catch_unwind` で `Escalate` に変換）の責務である（RFC 0009）。

## 4. 不変条件

- **INV-SUP-1**: 子の登録は `Create` 送信より先に完了している（SUP-2 / AC-H4）。したがって「親が知らない子」の失敗通知は発生しない。
- **INV-SUP-2**: `Terminating` 状態の `to_die` が空になる瞬間は高々 1 回であり、その瞬間の reason 観測は原子的である（SUP-5 の単一操作により成立）。
- **INV-SUP-3**: `FailedInfo::Fatal` は `clear_failed` 以外で解除・降格されない（Fatal 優先ガード）。
- **INV-SUP-4**: restart の実行回数は時間窓内で `RestartLimit` を超えない（SUP-9 の check+apply 原子化により成立）。
- **INV-SUP-5**: `finish_recreate` / `finish_terminate` は「全子が終了済み」の状態でのみ実行される（deferred 機構と `ChildrenContainer` の遷移により成立）。
- **INV-SUP-6**: 失敗した actor の user メッセージ処理は、supervisor の裁定（Resume / Restart 完了）まで再開されない（report_failure の suspend と mailbox の suspend カウンタにより成立）。

## 5. 機械的な問いへの回答

- **エラー時の倒れ先は?** — decider 未定義のエラーは既定 decider により分類ベース（Recoverable → Restart）で倒れる。`report_failure` で親が不在の場合は `Stop`（fail-close）。
- **同時に 2 つ来たら?**（複数の子が同時に失敗）— `Failure` は親の system queue で直列化される。`AllForOne` の 2 発目は既に Terminating の子に対して `shall_die` の追加のみが起き、`ChildrenContainer` が合流を調停する。
- **否定をかけるとどう化ける?** — `Kill` は「stop の強い版」ではなく「自傷の failure」である（SystemMessage 表）。stop 系と supervision 系は経路が異なる。
- **このループは何で止まる?** — restart ループは `RestartLimit` + 時間窓で止まる（INV-SUP-4）。escalate の連鎖は guardian（RFC 0005）の戦略（root は常に Stop）で止まる。

## 6. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-SUP-1 | `SupervisorOptions` が定義されているが `Props` に配線されておらず、戦略は常に `Actor::supervisor_strategy` の動的解決（SUP-12） | Props 経由の静的指定は将来計画か、削除すべき未使用型か | spawn 時に戦略を固定したい利用者の API 期待 |
| OQ-SUP-2 | `Resume` の子伝播が無条件で、Pekko の perpetrator 限定 resume と異なる（SUP-7） | 意図的な簡略化か、追従予定の差分か | AllForOne + Resume 時に無関係な子まで resume される |
| OQ-SUP-3 | `PoisonPill` と `Stop` が完全に同一ハンドラ（SUP-1）。Pekko では PoisonPill は user queue 順で処理される点が異なる | user 経由 / system 経由の両モードを持つ現設計で、Pekko の「キュー内の位置」セマンティクスをどこまで保証するか | 停止タイミングの互換性 |

形式化候補（Lean）: `ChildrenContainer`（4 状態 + 4 遷移）と `FailedInfo` × mailbox suspend カウンタの積状態機械。INV-SUP-5（全子終了後にのみ finish_*）は「子の集合 + DeathWatchNotification のインターリーブ」に対する到達可能性の定理として、INV-SUP-4 は時間窓付きカウンタの不変条件としてモデル化する価値が高い。

## 7. 参照

- Pekko: `ChildrenContainer.scala` / `FaultHandling.scala` / `Actor.scala`（実装コメントに対応表・行番号）
- ADR 0002、RFC 0002 / 0003 / 0005
