# RFC 0002: メッセージングと mailbox

| 項目 | 内容 |
|------|------|
| Status | As-built |
| 対象コード | `modules/actor-core-kernel/src/dispatch/mailbox/`, `modules/actor-core-kernel/src/actor/messaging/`, `modules/actor-core-kernel/src/actor/actor_ref/dead_letter/` |
| 関連文書 | RFC 0003（配送経路）, RFC 0007（Dead Letter の観測）, `docs/gap-analysis/actor-mailbox-gap-analysis.md`, `CONTEXT.md`（Mailbox Resolution / Dead Letter / Bounded Mailbox Compatibility） |
| 最終照合日 | 2026-07-11 |

## 1. 用語

Mailbox Resolution (メールボックス解決)、Dead Letter (デッドレター)、Bounded Mailbox Compatibility。ほかに本 RFC 固有の実装語として Envelope（搬送単位）、system queue / user queue（mailbox 内の 2 系統キュー）を用いる。

## 2. 概要

メッセージは `AnyMessage`（型消去コンテナ）として作られ、`Envelope` に包まれて mailbox の user queue へ入る。カーネル制御メッセージ（`SystemMessage`、RFC 0004）は独立した system queue へ入る。mailbox は `MailboxScheduleState`（`AtomicU32` 1 語のビット状態機械）でスケジュール状態を管理し、dispatcher（RFC 0003）が `Mailbox::run` を実行して両キューを排出する。

## 3. 規範仕様

### 3.1 メッセージ表現（宣言された挙動）

- **MB-1.** `AnyMessage` の payload は `ArcShared<dyn Any + Send + Sync + 'static>` であり、受信側は `downcast_ref::<T>()` で復元する（`actor/messaging/any_message.rs`）。island を跨ぐ値と同様、mailbox を通る値は `Send + Sync` でなければならない（MUST）。
- **MB-2.** sender は `Envelope` ではなく `AnyMessage` の `sender: Option<ActorRef>` フィールドで運ばれる。`Envelope` は payload のみを持つ薄いラッパーであり、priority / correlation 等のメタデータを意図的に持たない（`dispatch/mailbox/envelope.rs` の rustdoc に「必要になった時に追加する」と宣言されている）。
- **MB-3.** `AnyMessage` は 2 つのフラグを持つ: `is_control`（control-aware mailbox で優先される。`AnyMessage::control` で設定）と `not_influence_receive_timeout`（Receive Timeout をリセットしない。`AnyMessage::not_influence` のみが設定でき、`NotInfluenceReceiveTimeout` trait 境界をコンパイル時に要求する）。

### 3.2 メッセージキュー（宣言された挙動）

- **MB-4.** `MessageQueue` 実装は **10 種**（bounded / unbounded × plain / deque / priority / stable-priority / control-aware）であり、`MailboxType` ファクトリが 1:1 で対応する（`dispatch/mailbox/`）。
- **MB-5.** bounded キューの満杯時挙動は `MailboxOverflowStrategy` の 3 値で決まる（MUST）:
  - `DropNewest` — 新しい envelope を拒否（`EnqueueOutcome::Rejected`）
  - `DropOldest` — 最古の envelope を追い出して受理（`EnqueueOutcome::Evicted`）
  - `Grow` — 常に受理
- **MB-6.** `Evicted` / `Rejected` は**送信者から見れば成功**として扱わなければならない（MUST）。あふれた envelope の Dead Letter 記録は mailbox 層が単独で行う（`enqueue_outcome.rs` rustdoc。Pekko `BoundedNodeMessageQueue.enqueue` の void-on-success と等価）。`EnqueueError` は closed / timeout 等の真の失敗のみを表す。
- **MB-7.** 優先度キューのうち stable-priority 系のみが同一優先度内の FIFO を保証する（挿入順 `sequence` による全順序、`stable_priority_entry.rs`）。plain priority 系は同一優先度内の順序を保証しない（MUST NOT に依存しないこと）。
- **MB-8.** control-aware bounded キューの `DropOldest` は **normal queue の先頭のみ**を追い出す。normal queue が空のときは control メッセージであっても `Rejected` になる（`bounded_control_aware_message_queue.rs`、design Decision 3 として宣言済み）。
- **MB-9.** deque（stash 用）の `enqueue_first`（先頭挿入）は `DropOldest` でも evict せず `Reject` する。先頭を evict すると push_front 直後の envelope 自身を捨てる矛盾が生じるため（`bounded_deque_message_queue.rs`、design Decision 2-c として宣言済み）。

### 3.3 mailbox 本体（宣言された挙動）

- **MB-10.** system queue は user queue より常に優先されなければならない（MUST）。`run()` はまず system を全排出し、さらに **user メッセージ 1 件処理するごとに** system を全排出する（`base.rs`。Pekko `Mailbox.scala:274` parity）。
- **MB-11.** `throughput` は user メッセージ専用の件数上限であり、system 排出はこれを消費しない。`throughput_deadline` は mailbox に単調クロック（`MailboxClock`）が設定されている場合のみ有効で、`run()` 冒頭に一度だけ期限を計算し、超過で user 処理を打ち切る。
- **MB-12.** suspension は **dequeue のみを阻止**し、enqueue は常に受理されなければならない（MUST）。suspend 中の actor も受信メッセージをバッファし、resume 後に観測する（`base.rs` の rustdoc に Pekko parity として明記）。
- **MB-13.** suspend 中でも system メッセージの処理は継続する（`Resume` / `Stop` / `Watch` / failure 処理は user 側が suspend されていても配送される必要があるため。`can_be_scheduled_for_execution` の実装と注記）。
- **MB-14.** `put_lock` が原子性を守る複合操作は次の 3 つに限られ、単発の dequeue / メトリクス読み取りは内部キューの排他で足りる（`base.rs` 冒頭コメントで宣言）: `enqueue_envelope_locked`（is_closed 確認 + enqueue）、`prepend_user_messages_deque_locked`（is_closed 確認 + 先頭一括挿入）、`finalize_cleanup`（drain + clean_up + finish_cleanup）。
- **MB-15.** user メッセージの dequeue 直前には closed / suspended の再チェックを行う。これは呼び出し側ループ条件との**意図的な重複**であり、`Suspend` / `Close` が別スレッドから割り込む TOCTOU への防御である（`base.rs` の「Do not remove the duplication」コメント）。この重複を除去してはならない（MUST NOT）。

### 3.4 暗黙の挙動

- **MB-16.** 既定の mailbox は unbounded（`MailboxConfig::default()` → `MailboxPolicy::unbounded(None)`、実体は lock-free MPSC キュー）。既定 mailbox の登録 ID は `"fraktor.actor.default-mailbox"`（Pekko の `pekko.actor.default-mailbox` とは意図的に異なる文字列）。
- **MB-17.** mailbox の解決優先順位（Mailbox Resolution）は次の 3 段（`actor/actor_cell.rs` の `create`）:
  1. dispatcher の `try_create_shared_mailbox`（`BalancingDispatcher` のみ Some を返す）
  2. `Props` の `mailbox_id` 指定
  3. `Props::mailbox_config()` からのファクトリ生成
  さらにキュー種別の選択は 5 段の優先順位（priority_generator + stable → stable-priority、priority_generator → priority、control-aware 要求、deque 要求、既定の capacity 依存）で決まる（`mailboxes.rs`）。
- **MB-18.** `MailboxPolicy::unbounded` は overflow に `DropOldest` を設定するが、unbounded キューが選ばれた時点でこの値は使用されない（到達しない設定値）。

## 4. 状態機械: `MailboxScheduleState`

`AtomicU32` 1 語にパックされたビット状態機械（`dispatch/mailbox/schedule_state.rs`）。

状態要素: `SCHEDULED` / `RUNNING` / `CLOSE_REQUESTED` / `FINALIZER_OWNED` / `CLEANUP_DONE` の 5 フラグ + suspend カウンタ（第 5 ビット以降）+ `need_reschedule` 補助フラグ。

| 遷移関数 | 事前条件 | 事後条件 |
|---------|---------|---------|
| `request_schedule(hints)` | 作業あり（system / user / backpressure のいずれか）。close 済み・cleanup 済みなら失敗。suspend 中は system ヒントがある場合のみ | `SCHEDULED` セットで `true`。既に `SCHEDULED\|RUNNING` なら `need_reschedule` を立てて `false` |
| `set_running()` | scheduled | `SCHEDULED` → `RUNNING` |
| `set_idle()` | running | `RUNNING\|SCHEDULED` クリア。`need_reschedule` を swap して返す |
| `suspend()` / `resume()` | — | suspend カウンタ ±1（ネスト対応、resume は 0 で飽和） |
| `request_close()` | — | `CloseRequestOutcome` を返す（下記） |
| `finish_run()` | running | `RunFinishOutcome` を返す（下記） |
| `finish_cleanup()` | finalizer 所有 | `CLEANUP_DONE` セット |

- `CloseRequestOutcome`（4 値）: `CallerOwnsFinalizer`（呼び出し元が finalize を実行）/ `RunnerOwnsFinalizer`（実行中の runner が finish_run 経由で実行）/ `AlreadyRequested` / `AlreadyCleaned`。
- `RunFinishOutcome`（3 値）: `Continue { pending_reschedule }` / `FinalizeNow` / `Closed`。

`Mailbox::run` の全体順序: cleanup 済みなら即終了 → `set_running` → close 未要求なら system 全排出 → user 処理（throughput / deadline 制御、1 件ごとに system 割込み排出）→ `finish_run` の結果で継続・終端処理・終了を分岐。

## 5. 不変条件

- **INV-MB-1**: suspension は dequeue のみを阻止し、enqueue を失敗させない（MB-12）。
- **INV-MB-2**: user メッセージを 1 件処理する前に、system queue は必ず空になっている（MB-10）。
- **INV-MB-3**: あふれによる `Evicted` / `Rejected` の Dead Letter 記録者は mailbox 層ただ一つであり、送信者側は成功を観測する（MB-6）。二重記録は発生しない。
- **INV-MB-4**: 呼び出し側が `Suspend` / `Close` を確定した後に user メッセージが消費されることはない（MB-15 の二重チェックにより成立）。
- **INV-MB-5**: close の finalizer 所有者は常に一意である（`FINALIZER_OWNED` フラグと `CloseRequestOutcome` / `RunFinishOutcome` の CAS 遷移により成立）。
- **INV-MB-6**: stable-priority キューでは、同一優先度のメッセージは enqueue 順に dequeue される（MB-7）。
- **INV-MB-7**: system queue への enqueue は常に成功する（`enqueue_system` は `Ok(())` を返す。Treiber スタック + draining フラグによる FIFO 復元）。

## 6. 機械的な問いへの回答

- **空/未設定のとき何が起きる?** — 空キューの dequeue は `None`（エラーではない）。mailbox 未指定の spawn は既定 unbounded mailbox に倒れる（MB-16）。
- **エラー/取得失敗のとき?** — enqueue の真の失敗は `SendError`（6 値: `Full` / `Suspended` / `Closed` / `NoRecipient` / `Timeout` / `InvalidPayload`）として返り、`DeadLetterReason` へ機械的に写像される（§7）。
- **境界はどっち向き?** — bounded 判定は `len >= capacity` で「新規を入れる前」に評価する（capacity ちょうどで満杯）。
- **同時に 2 つ来たら?** — ロック系キューは write lock で直列化。unbounded user queue はロックフリー MPSC（in-flight カウンタで close との競合を検出）。close とファイナライズの競合は `put_lock` と `CloseRequestOutcome` の CAS が調停する。
- **この値は誰が決める?** — capacity / overflow / 優先度生成器は `Props`（`MailboxConfig`）が決め、throughput / deadline は dispatcher が決める（RFC 0003）。
- **2 つのシステムが合意しているか?** — 送信側（`try_tell` の失敗記録）と mailbox 層（あふれ記録）の Dead Letter 記録責務は排他に分割されており、`try_tell` に到達する失敗は closed / timeout 等の真の失敗のみ（RFC 0003 参照）。

## 7. Dead Letter 分類

`DeadLetterReason` は 10 値（`actor/actor_ref/dead_letter/dead_letter_reason.rs`）。`SendError` からの写像は `DeadLetter::record_send_error` が単一の変換点。

| Reason | 記録元 |
|--------|--------|
| `MailboxFull` | mailbox 層のあふれ記録、および `SendError::Full` の写像 |
| `MailboxSuspended` / `MailboxTimeout` / `RecipientUnavailable` / `MissingRecipient` / `SerializationError` | `SendError` 各 variant の写像（`SerializationError` は serialization extension からの直接記録もある） |
| `ExplicitRouting` | `LocalActorRefProvider` |
| `Dropped` | mailbox `finalize_cleanup` の残留メッセージ記録 |
| `FatalActorError` / `SuppressedDeadLetter` | **kernel 内に記録元が存在しない（予約 variant）** → OQ-MB-1 |

既定バッファ容量: Dead Letter 単体の既定は 256、システム構築時は 512 が明示指定される（RFC 0007 参照）。

## 8. Open Questions

| # | 観測した事実 | 質問 | 影響 |
|---|-------------|------|------|
| OQ-MB-1 | `DeadLetterReason::FatalActorError` と `SuppressedDeadLetter` は定義のみで、kernel 内に記録元がない | 将来の予約か、配線漏れか（特に suppression は Pekko の `DeadLetterSuppression` marker に相当する仕組み自体が未実装。RFC 0007 OQ-EV-2 参照） | Dead Letter 分類の網羅性・観測の意味論 |
| OQ-MB-2 | `MailboxPolicy::unbounded` が到達しない `DropOldest` を設定している（MB-18） | 誤解を招く既定値を `Grow` 等へ正規化すべきか | 実害はないが、設定値と実挙動の乖離 |
| OQ-MB-3 | plain priority キューは同一優先度内の順序が不安定（MB-7） | 利用者が stable を選ぶべき場面のガイドが公開面にあるか | 順序に依存した利用のバグ温床 |

形式化候補（Lean）: `MailboxScheduleState` は「5 フラグ + カウンタ + 8 遷移関数」の有限状態機械であり、INV-MB-4（Suspend/Close 後の user 消費なし）と INV-MB-5（finalizer 一意性）は Lean の inductive 状態 + 遷移関数でモデル化し、全遷移列に対する定理として証明する価値が高い。`CloseRequestOutcome` の 4 分岐は並行 close 要求の合流点であり、反例探索の主対象。

## 9. 参照

- Pekko: `Mailbox.scala`（run loop / suspension / TOCTOU ガードの対応行が実装コメントに記載）
- RFC 0003（dispatcher からの `run` 呼び出しと再スケジュール）、RFC 0004（SystemMessage の処理側）、RFC 0007（Dead Letter / mailbox メトリクスの観測）
- `docs/gap-analysis/actor-mailbox-gap-analysis.md`
