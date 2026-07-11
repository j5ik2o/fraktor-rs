# RFC 0003: dispatch と executor

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/dispatch/dispatcher/`, `modules/actor-core-kernel/src/actor/actor_ref/base.rs`, `modules/actor-core-kernel/src/actor/actor_ref/actor_ref_sender_shared.rs` |
| 関連文書 | RFC 0002（mailbox）, RFC 0009（Executor の adaptor 実装）, `CONTEXT.md`（Kernel Public Surface） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

dispatcher（メッセージの enqueue と実行スケジュールの調停者）、executor（タスクを実行環境へ載せる port）、drain owner（トランポリンキューの排出権を持つ呼び出し元）。

## 2. 概要

送信経路は「`ActorRef::tell` → `DispatcherSender::send`（Envelope 化）→ `dispatcher.dispatch_enqueue`（mailbox へ enqueue）→ `register_user_candidates`（実行登録）」、実行経路は「`register_for_execution` → `executor.execute` → `Mailbox::run` → 必要なら再スケジュール」である。設計の中心は**再入（re-entrancy）安全性**にあり、二段階送信・トランポリン・drive guard の 3 機構で「メッセージハンドラ内からの `tell` / spawn / stop」が呼び出しスタック上でデッドロックや意図しないネスト実行を起こさないことを保証する。

## 3. 規範仕様

### 3.1 tell の契約（宣言された挙動）

- **DISP-1.** `ActorRef::tell` は fire-and-forget であり、**at-most-once** 配送である（MUST）。失敗は Dead Letter / 観測経路に記録され、呼び出し元には現れない（`actor/actor_ref/base.rs` rustdoc。Pekko の tell セマンティクスと一致）。
- **DISP-2.** 配送失敗を呼び出し元で観測したい場合は `try_tell` を使わなければならない（MUST）。`try_tell` に到達する失敗は closed / timeout / missing recipient / serialization といった**真の失敗**のみであり、mailbox あふれ（Evicted / Rejected）は mailbox 層が記録済みのため成功として返る（RFC 0002 INV-MB-3 と対）。
- **DISP-3.** `tell` は `--cfg fraktor_disable_tell` で無効化できる。これは「エラーを暗黙に握りつぶす経路」を排除したビルドが成立することを検証する CI 契約である（`Makefile.toml` の actor-tell-disabled-check）。
- **DISP-4.** `AnyMessage` → `Envelope` の変換は `DispatcherSender::send` 内で行われる。`ActorRef` 層は Envelope を知らない。

### 3.2 二段階送信（宣言された挙動）

- **DISP-5.** 送信は 2 フェーズに分割されなければならない（MUST）:
  1. `dispatch_enqueue` — per-actor sender ロック内で mailbox へ enqueue し、実行候補 mailbox のリストを返す
  2. `register_user_candidates` — sender ロック**解放後**に実行登録する
  理由: inline executor では `register_for_execution` 内の `executor.execute` が同期的に `Mailbox::run` を走らせる。これを sender ロック内で行うと、メッセージハンドラからの再入 `tell` が同じ per-actor sender mutex に対してデッドロックする（`dispatcher_sender.rs` / `message_dispatcher_shared.rs` の rustdoc に宣言）。
- **DISP-6.** 一括版 `MessageDispatcherShared::dispatch` を無関係なロックを保持したまま呼んではならない（MUST NOT）。inline executor 下ではネストした `mailbox.run` が同じロックへ再入しうる（同 rustdoc の警告）。

### 3.3 実行登録と再スケジュール（宣言された挙動）

- **DISP-7.** `register_for_execution` は `ScheduleHints` を組み立てて `mailbox.request_schedule` を試み、成功時のみ `executor.execute` にクロージャを積む。クロージャは `Mailbox::run(throughput, throughput_deadline)` を実行し、戻り値（pending reschedule）が真なら**自分自身を再登録**しなければならない（MUST）。再登録しなければ、run 中に到着したメッセージは「mailbox が running だったため schedule せず返った tell」によって誰にも起こされず滞留する（`base.rs` rustdoc に宣言）。
- **DISP-8.** `executor.execute` が失敗した場合、呼び出し元は mailbox のスケジュール状態を idle にロールバックしなければならない（MUST。`Executor` trait rustdoc に宣言、`register_for_execution` が実装）。
- **DISP-9.** `affinity_key` は mailbox の PID 値であり、affinity 対応 executor は同一 mailbox を同一 worker へ安定的にルーティングする。非対応 executor はこの値を無視してよい（MAY）。
- **DISP-10.** 複数候補（`BalancingDispatcher`）の実行登録は優先順に試行し、最初に成功した時点で打ち切る。

### 3.4 トランポリンと drive guard（宣言された挙動）

- **DISP-11.** `ExecutorShared::execute` は「pending キューへ push → `running` フラグの CAS に勝った呼び出し元だけが drain owner としてキューを排出」という構造を取る。内部 executor のロックは各 `inner.execute` 呼び出しの間だけ保持し、drain ループ全体では保持しない（再入デッドロック防止、`executor_shared.rs` モジュール doc）。
- **DISP-12.** `DriveGuardToken` の `Drop` で pending キューの tail drain を行ってはならない（MUST NOT）。Drop で同期排出すると、子 mailbox が呼び出し元スタック上で実行される——この機構が防ぎたい再入そのものになる（`drive_guard_token.rs` rustdoc に理由が宣言されている）。残タスクは次の `execute` 呼び出しが自然に回収する。
- **DISP-13.** `run_with_drive_guard` は、`pre_restart`（`stop_all_children` が子 mailbox へ同一スレッド再入しうる）のような区間を drive guard 配下で実行するための API である（RFC 0004 の fault_recreate から使用される）。

### 3.5 dispatcher 実装 3 種（宣言された挙動）

| 実装 | 特性 |
|------|------|
| `DefaultDispatcher` | `DispatcherCore` のみの素の共有 dispatcher（Pekko `Dispatcher` 相当） |
| `PinnedDispatcher` | 1 actor 専属。構築時に `throughput = usize::MAX`, `deadline = None` へ正規化。別 actor の register は `SpawnError::DispatcherAlreadyOwned`（Pekko `PinnedDispatcher.scala` parity） |
| `BalancingDispatcher` | 共有キューを `try_create_shared_mailbox` で全 team に配り、dispatch 時は primary + 生存 team 全員を実行候補として返す（busy receiver からのフォールバック） |

- **DISP-14.** `MessageDispatcher` trait はクエリを `&self`、コマンドを `&mut self` とする CQS 契約を持ち、`register_for_execution` は意図的に trait から排除され `MessageDispatcherShared` のみが持つ（trait フック内での再ロック事故防止。`message_dispatcher.rs` モジュール doc に宣言）。

### 3.6 暗黙の挙動

- **DISP-15.** 既定値: `throughput = 5`、`throughput_deadline = None`、`shutdown_timeout = 1 秒`（`DispatcherConfig::with_defaults`）。
- **DISP-16.** 既定 dispatcher（`fraktor.actor.default-dispatcher`）の executor は `InlineExecutor`（呼び出しスレッドで同期実行、自前トランポリン内蔵）である。rustdoc は「deterministic tests 用」と述べるが、実際には `ActorSystemConfig::default()` の本番既定経路である → OQ-DISP-1。
- **DISP-17.** `throughput_deadline` の実施は dispatcher ではなく mailbox 側で行われる（RFC 0002 MB-11）。dispatcher は値を保持・伝搬するだけである。`BalancingDispatcher` は共有 mailbox 生成時に system-wide の `MailboxSharedSet`（deadline 用クロックを内包）を使う必要があり、builtin セットを使うと deadline enforcement が無効化される（実装コメントに宣言）。
- **DISP-18.** `ExecuteError` は 3 値: `Rejected`（キュー飽和等）/ `Shutdown` / `Backend(String)`。`ExecutorShared::shutdown` は内部 executor の shutdown に加えて pending キューを**破棄**する。

## 4. 状態機械

dispatch 層固有の状態機械は 2 つ（mailbox 側の `MailboxScheduleState` は RFC 0002 §4）:

- **drain owner CAS**: `running: AtomicBool` の compare_exchange(false→true) に勝った 1 スレッドのみが drain owner。敗者は push のみ。owner は解放後に 1 回だけ tail drain を行い、解放と push の競合を回収する。
- **`ShutdownSchedule`**（3 値: `Unscheduled` / `Scheduled` / `Rescheduled`）: dispatcher shutdown の遅延スケジュール状態。

## 5. 不変条件

- **INV-DISP-1**: `tell` の失敗が呼び出し元へ返ることはない。すべての失敗は Dead Letter / 観測経路に現れる（DISP-1、RFC 0002 §7 と合わせて配送失敗の観測完全性を構成する）。
- **INV-DISP-2**: per-actor sender ロックを保持したまま `Mailbox::run` が同期実行されることはない（DISP-5 の二段階分離により成立）。
- **INV-DISP-3**: 任意の時点で drain owner は高々 1 つである（running フラグの CAS により成立）。
- **INV-DISP-4**: `Mailbox::run` が pending reschedule を報告した場合、必ず再登録が行われる（DISP-7。これが破れるとメッセージが無期限滞留する）。
- **INV-DISP-5**: `PinnedDispatcher` に同時に register できる actor は高々 1 つである。

## 6. 機械的な問いへの回答

- **エラー/取得失敗のとき true か false か例外か?** — `register_for_execution` は bool を返し、失敗（executor 拒否）時は mailbox 状態をロールバックして false。呼び出し元へ例外的伝播はしない。
- **同時に 2 つ来たら?** — 同一 mailbox への同時 `request_schedule` は CAS で一方だけが勝ち、他方は `need_reschedule` を立てる。同時 `execute` は drain owner CAS で直列化される。
- **このループ/引き上げは何で止まる?** — drain ループは pending キューが空になった時点で止まる。run → 再登録の連鎖は「mailbox に作業がない」（`request_schedule` が false）で止まる。
- **2 つのシステムがこのデータで合意しているか?** — 「schedule 済みかどうか」は mailbox の `MailboxScheduleState` が唯一の真実であり、dispatcher 側に複製状態を持たない（合意問題を発生させない設計）。

## 7. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-DISP-1 | `InlineExecutor` は rustdoc で test 用と宣言されつつ、既定 dispatcher の本番経路である（DISP-16） | 既定を std adaptor の executor に差し替える予定か、no_std 既定として意図的か。rustdoc の記述と実態のどちらが正か | 既定構成での並行実行の有無、ドキュメントの信頼性 |
| OQ-DISP-2 | `run_with_drive_guard` の適用箇所は fault_recreate 経路のみ | 他の「ハンドラ内から子 mailbox を同期駆動しうる」経路（stop_all_children を直接呼ぶ利用者コード等）は guard なしで安全か | 再入防止の被覆の穴 |

形式化候補（Lean）: 「drain owner CAS + pending キュー + need_reschedule」の 3 者からなる並行プロトコル。INV-DISP-3（owner 一意性）と INV-DISP-4（作業のロストなし: すべての enqueue はいつか run される）は、tell / run / execute のインターリーブに対する安全性・活性の定理としてモデル化する価値が高い（gist の「カウンタの不変条件」「収束/runaway」パターンに対応）。

## 8. 参照

- Pekko: `Dispatcher.scala` / `PinnedDispatcher.scala` / `BalancingDispatcher`（実装コメントに対応行）
- RFC 0002（mailbox run / schedule state）、RFC 0009（ThreadedExecutor / AffinityExecutor / TokioExecutor / EmbassyExecutor の port 実装）
