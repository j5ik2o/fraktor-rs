## Context

`MailboxOverflowStrategy::Block` を中核とする bounded mailbox の async backpressure 機能群は、次の経緯で半実装の dead debt になっている。

1. **Phase 11.8 (`abbc26313`)**: `feat(dispatcher): route MailboxOfferFuture through DispatcherWaker` で `NewDispatcherSender::drive_offer_future` を導入。意図は「`MailboxOfferFuture` を polling し、Pending のときは `register_for_execution` で receiver を drain させて capacity を空ける」だったが、実装は `loop { match poll { Pending => register_for_execution; continue } }` という busy-loop。InlineExecutor では `register_for_execution` が同期 drain するため数イテレーションで終わって偶然動作したが、非 inline executor では caller thread が spin する。この挙動を end-to-end でテストする経路は **一度も実装されていなかった** (`mailbox_offer_future` の単体テスト 4 件は queue を直接叩いて手動で dequeue する経路で、`drive_offer_future` を通らない)
2. **PR #1525 マージ後 Bugbot レビュー**:
   - `#3043806318` (Medium): `drive_offer_future` の busy-loop を指摘
   - `#3043448944` (Medium): `dispatcher_waker` の wake hint inversion を指摘 (これ自体は別バグだが結果的に dead code 内の修正となった)
3. **修正コミット `6ce9b357`**: `drive_offer_future` を完全に削除し、`MessageDispatcher::dispatch` default impl が `EnqueueOutcome::Pending(future)` を `SendError::full(payload)` で即返すように変更。busy-loop は解消したが Block の "wait for capacity" semantics は完全に失われ、`dispatcher_waker` モジュールが production caller ゼロの dead code となった
4. **Bugbot 追加指摘**: `dispatcher_waker is dead code` (`#3043448944` の波及確認)

つまり Block strategy は **設計** (`design.md §7 async backpressure と DispatcherWaker`) と **実装** (`drive_offer_future` busy-loop) と **テスト** (end-to-end 検証なし) の 3 点すべてで未完成だった。

参照実装の状況:

- **Apache Pekko**: `BoundedMailbox` は `LinkedBlockingQueue` で thread parking する `pushTimeOut` semantics を持つが、Pekko 自身が `Mailboxes.scala:259` で warn ログを出して非推奨化。`NonBlockingBoundedMailbox` (overflow → DeadLetters) を推奨パスとしている
- **Proto.Actor Go**: `actor/bounded.go` 67 行のうち、`Bounded(N)` は Workiva ring buffer の Put 失敗を log して捨てる lossy 実装、`BoundedDropping(N)` は drop-oldest。**thread parking する semantics は存在しない**
- **fraktor-rs (no_std + multi-target)**: thread parking primitive が `fraktor-utils-rs` に存在しない。実装するには std / tokio / embedded で別々の primitive を抽象化する必要があり、設計コストが高い

## Goals / Non-Goals

**Goals:**
- `MailboxOverflowStrategy::Block` を完全撤去し、`DropNewest` / `DropOldest` / `Grow` の 3 戦略のみを残す
- `MailboxOfferFuture` / `EnqueueOutcome::Pending` / `dispatcher_waker` を含む async backpressure 関連の半実装を 1 PR で根こそぎ削除
- `Executor::supports_blocking()` trait method と spawn 時の `supports_blocking()` ゲートを削除し、executor 抽象を小さくする
- `Mailbox::enqueue_envelope` の戻り値を `Result<(), SendError>` に簡素化
- 削除に伴う openspec spec delta を `dispatch-executor-unification` と `dispatcher-attach-detach-lifecycle` に反映する
- 削除作業を **コミット粒度で段階化** (1 コミット = 1 削除 cluster) し、各コミットで `cargo test` が通る状態を保つ

**Non-Goals:**
- 別の sync blocking primitive の追加 (`fraktor-utils-rs` への `Blocker` port、condvar wrapper、thread parking 抽象など)
- async send variant (`try_tell_async`) の設計・追加
- stream module の backpressure 機構 (`fraktor-utils-rs::OverflowPolicy::Block`, `WaitShared`) の変更
- `MailboxOverflowStrategy` の他 variant (`DropNewest` / `DropOldest` / `Grow`) の semantics 変更
- 既存の dispatcher / executor 抽象の AShared 構造変更
- bounded mailbox に新しい overflow 戦略を追加すること

## Decisions

### 1. Block を撤去し、置き換え機能を追加しない

`MailboxOverflowStrategy::Block` を撤去し、別の sync block primitive を新規追加しない。理由:

- production caller がゼロ
- 参照実装 (Pekko / Proto.Actor Go) のいずれも fraktor-rs にとっての必須機能として位置付けていない (Pekko は非推奨、Proto.Actor Go は非実装)
- 実装するには `fraktor-utils-rs` に no_std 互換の thread parking 抽象を追加する必要があり、実需要ゼロのために投資する価値がない

代替案:

- **Pekko 風 sync block を実装する** (`fraktor-utils-rs::Blocker` port 追加 + std/tokio/embedded で別実装)
  - 却下理由: production caller がゼロ。Pekko 自身が非推奨にしている機能を no_std + multi-target で実装する投資対効果が悪い
- **async send variant `try_tell_async` を追加する**
  - 却下理由: 同じく実需要ゼロ。必要になった時点で別 change として提案すれば、その時点で実 caller を持って設計・テストできる
- **`#[deprecated]` でマークして段階的に撤去**
  - 却下理由: `CLAUDE.md` の「後方互換不要」「pre-release phase」「破壊的変更を歓迎」方針に反する。`legacy-code-temporary-usage.md` の「PR/タスク完了時にレガシー実装を残さない」原則とも衝突する。deprecation marker を入れる作業自体が新たな作業を生み、scope 削減効果も限定的

### 2. `Mailbox::enqueue_envelope` の戻り値を簡素化する

`MailboxOverflowStrategy::Block` 撤去によって `EnqueueOutcome::Pending` variant が不要になり、`EnqueueOutcome` enum の variant が `Enqueued` 1 つだけになる。enum 自体を撤去し、`enqueue_envelope` の戻り値を `Result<(), SendError>` に簡素化する。

代替案:

- **`EnqueueOutcome` enum を残し、variant を `Enqueued` のみにする**
  - 却下理由: 1 variant の enum は意味のない indirection。caller 側に `match` を強制する負荷だけが残る
- **戻り値を `Result<usize, SendError>` (enqueued count) などに拡張する**
  - 却下理由: YAGNI。現在の caller は成功 / 失敗を区別できれば十分で、count を必要とする use case がない

### 3. `Executor::supports_blocking()` trait method を撤去する

唯一の caller である `MessageDispatcherShared::attach` 内の Block strategy compatibility check が削除されるため、`supports_blocking()` 自体も不要になる。trait method、`ExecutorShared` の convenience method、各具象 executor (`InlineExecutor` / `TokioExecutor` / `ThreadedExecutor` / `PinnedExecutor`) の impl をすべて削除する。

代替案:

- **trait method は残し、default `true` にしておく**
  - 却下理由: 誰も呼ばない trait method を残すのは dead code。executor abstraction が無用に大きくなる。`legacy-code-temporary-usage.md` の原則に反する
- **`#[allow(dead_code)]` で残す**
  - 却下理由: 同上 + lint 抑止が design debt を隠す

### 4. `SpawnError::InvalidMailboxConfig` variant は完全削除する

`SpawnError::InvalidMailboxConfig` variant + `invalid_mailbox_config()` constructor は `MailboxOverflowStrategy::Block` 関連エラーを表すために定義されたが、**現コードに caller がゼロ**である (`grep "InvalidMailboxConfig\|invalid_mailbox_config"` の結果、定義 (`spawn_error.rs:28`, `spawn_error.rs:62`) と rustdoc (`actor_cell.rs:157`) 以外の参照なし)。

`MailboxOverflowStrategy::Block` 撤去後は将来的にも使われる予定がないため、variant + constructor を完全削除する。残すと dead code になる。

代替案:

- **variant は残し、Block 関連 doc だけ削除する**
  - 却下理由: caller ゼロの dead enum variant を残すのは `legacy-code-temporary-usage.md` 原則違反
- **variant の意味を「他の汎用 mailbox 構成エラー」に再解釈して残す**
  - 却下理由: YAGNI 違反、想定 caller がない

### 5. Spec delta は `dispatcher-pekko-1n-redesign` の archive 後 baseline に対して書く

本 change の `specs/` は `dispatcher-pekko-1n-redesign` が archive されて baseline (`openspec/specs/`) に統合された後の状態に対して REMOVED / MODIFIED Requirements を定義する。`dispatcher-pekko-1n-redesign` は現状 138/140 タスク完了で archive 待ち状態にあり、Phase 14.5 / 14.5.1 は contention bench の follow-up で本機能と独立。

代替案:

- **`dispatcher-pekko-1n-redesign/specs/` を in-place で修正**
  - 部分却下理由: 既に PR #1525 でマージされた change の spec delta を後付けで書き換えるのは、change history の coherence を損なう。ただし archive がさらに大幅に遅延する場合は、本 change の spec delta を in-place 修正に切り替える代替パスを proposal で明示
- **本 change を `dispatcher-pekko-1n-redesign` に統合する**
  - 却下理由: スコープと意図が異なる (前者は Pekko 1:N model 移行、本 change は半実装機能の撤去)。混ぜると change の責務が曖昧になる

### 6. 削除作業をコミット粒度で段階化する

scope は中〜大 (約 35 ファイル、数百〜千行規模、`MessageQueue` trait 戻り値型変更による波及で全 queue 実装が対象) なので、PR 内で 5 コミットに分割する。各コミットは独立して `cargo test` が通り、独立して revert 可能。

**コミット順序の鍵**: `MailboxOverflowStrategy::Block` を先に削除して `EnqueueOutcome::Pending(future)` の唯一の生成元 (`bounded_message_queue::enqueue` の `Block` arm) を消し、その後 `EnqueueOutcome` enum 全体と `MessageQueue::enqueue` trait 戻り値型を簡素化する。これにより各 commit での「型が変わったが構築箇所が残っている」という不整合状態を回避できる。

```
commit 1: chore(mailbox): delete dead dispatcher_waker module
commit 2: feat(mailbox): remove MailboxOverflowStrategy::Block variant
commit 3: refactor(mailbox): simplify MessageQueue::enqueue to Result<(), SendError> and drop MailboxOfferFuture
commit 4: refactor(executor): remove supports_blocking trait method and SpawnError::InvalidMailboxConfig
commit 5: docs(openspec): mark Block / DispatcherWaker capabilities as removed
```

代替案:

- **1 コミットで一括削除**
  - 却下理由: 巨大 diff のレビュー負荷が高く、bisect で問題を切り分けにくくなる
- **5 コミットを別々の PR にする**
  - 却下理由: 中間状態 (例: `EnqueueOutcome::Pending` を消したが Block variant が残っている) は意味のない過渡状態であり、別 PR で公開する価値がない
- **`MailboxOfferFuture` 撤去を先に commit 2 に置く**
  - 却下理由: `bounded_message_queue.rs` の `Block` arm がまだ `MailboxOfferFuture::new(...)` を呼んでいる状態で `MailboxOfferFuture` を削除するとコンパイルが通らない。Block arm を先に消す必要がある

## Risks / Trade-offs

- **Risk**: 将来「actor mailbox の sync block backpressure が必要」という要件が出た場合、実装と spec を一から作り直す必要がある
  - **Mitigation**: その時点で実 caller を持って独立 change として提案できる。実装も Pekko / Proto.Actor Go の現時点の選択 (前者は非推奨、後者は非実装) を踏まえて、より正直な形で design できる。今 broken な状態で抱えるよりも将来的な負債は小さい
- **Risk**: 既存の `mailbox_offer_future/tests.rs` 4 件は queue 直叩きで future の挙動を検証していたが、これが消えることで「future が capacity 待ちで wake する」契約のテストカバレッジが失われる
  - **Mitigation**: `MailboxOfferFuture` 自体が消えるので、テストする対象もない。stream module 側の同等機構 (`WaitShared` ベース) は別レイヤーでカバーされている
- **Risk**: `dispatcher-pekko-1n-redesign` の archive がさらに遅延すると、本 change の spec delta が baseline と乖離する
  - **Mitigation**: proposal で sequencing 依存を明示。archive が遅延する場合は in-place 修正に切り替える代替パスを残す
- **Risk (重要)**: `dispatcher-pekko-1n-redesign/specs/dispatcher-attach-detach-lifecycle/spec.md:21-25` は「`MessageDispatcherShared::attach` が `MailboxOverflowStrategy::Block` の場合に `supports_blocking()` を検証する」と要求しているが、**現コードの `message_dispatcher_shared.rs:98-106` には当該ゲートが実装されていない**。spec が overstated な状態で baseline 化される
  - **Mitigation**: 本 change の spec delta が当該 scenario を MODIFIED で削除することで、spec が実装に追従する。実装側のタスクとしては「`attach` からゲートを削除する」ではなく「`actor_cell.rs:157` の死文 rustdoc を削除する」のみ。proposal の Sequencing セクションでこの状況を明示
- **Trade-off**: `MailboxOverflowStrategy` の variant 数が 4 → 3 に減ることで、`MailboxPolicy::bounded(...)` の caller がコンパイルエラーになる
  - **Mitigation**: production caller がゼロなのでコンパイルエラーはテストコードのみで発生する。テストコードの修正は機械的
- **Trade-off**: `MessageQueue::enqueue` trait 戻り値型変更により全 queue 実装 (10 ファイル + テスト) を一括修正する必要がある
  - **Mitigation**: 修正は完全に機械的 (`Ok(EnqueueOutcome::Enqueued)` → `Ok(())`)。コンパイラが取りこぼしを検出する。commit 3 でまとめて実施

## Migration Plan

各コミットは独立して `cargo test -p fraktor-actor-core-rs --lib` がグリーンになる状態を保つ。

### Commit 1: `chore(mailbox): delete dead dispatcher_waker module`

- 削除: `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_waker.rs`
- 削除: `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_waker/tests.rs` (とディレクトリ)
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher.rs` から `mod dispatcher_waker;` と `pub use dispatcher_waker::dispatcher_waker;` を削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:280-287` の `enqueue_envelope` rustdoc から `DispatcherWaker` 言及を削除
- 検証: `cargo check -p fraktor-actor-core-rs --lib --tests` + `cargo test -p fraktor-actor-core-rs --lib core::kernel::dispatch::dispatcher`

### Commit 2: `feat(mailbox): remove MailboxOverflowStrategy::Block variant`

`MailboxOfferFuture::new(...)` の唯一の production caller は `bounded_message_queue::enqueue` の `Block` arm。先にこれを消すことで commit 3 で `MailboxOfferFuture` を削除する際にコンパイルエラーが出ない。

- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/overflow_strategy.rs` から `Block` variant を削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/overflow_strategy/tests.rs` の `Block` test を削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue.rs` の `Block` match arm を削除 (この時点で `MailboxOfferFuture::new(...)` の唯一の production caller が消える)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs` の `Block` reject 分岐を削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs` の `Block` reject 分岐を削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue/tests.rs`, `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue/tests.rs`, `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue/tests.rs` の Block 関連テストを削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_handles.rs:94` の `Block => OverflowPolicy::Block` mapping を削除
- 修正: `MailboxPolicy::bounded(...)` を `MailboxOverflowStrategy::Block` で呼んでいる残テストを `DropNewest` 等に置換、または該当テストを削除
- 検証: `cargo check -p fraktor-actor-core-rs --lib --tests` + `cargo test -p fraktor-actor-core-rs --lib core::kernel::dispatch::mailbox`

### Commit 3: `refactor(mailbox): simplify MessageQueue::enqueue to Result<(), SendError> and drop MailboxOfferFuture`

`MessageQueue::enqueue` trait 戻り値型を `Result<EnqueueOutcome, SendError>` から `Result<(), SendError>` に変更し、`EnqueueOutcome` enum と `MailboxOfferFuture` を撤去する。trait 変更につき全 queue 実装に波及する commit (本 PR で最大の commit)。

#### trait + 共通型

- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/message_queue.rs` の `MessageQueue::enqueue` 戻り値を `Result<(), SendError>` に変更
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/deque_message_queue.rs` の `DequeMessageQueue::enqueue_first` 戻り値を `Result<(), SendError>` に変更
- 削除: `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_enqueue_outcome.rs` (`EnqueueOutcome` enum)
- 削除: `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_offer_future.rs` + `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_offer_future/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox.rs` (module declarations) から `pub use MailboxOfferFuture`、`pub use EnqueueOutcome`、`mod mailbox_enqueue_outcome`、`mod mailbox_offer_future` を削除

#### 全 queue 実装の追従

- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue.rs` (残り 3 strategy で `Ok(())` を返す)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs` (`Ok(EnqueueOutcome::Enqueued)` → `Ok(())`)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs` (同上)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_message_queue.rs` (同上)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_control_aware_message_queue.rs` (同上)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_priority_message_queue.rs` (同上)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_stable_priority_message_queue.rs` (同上)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_deque_message_queue.rs` (`enqueue` + `enqueue_first` 両方)
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/shared_message_queue.rs` (`Ok(EnqueueOutcome::Enqueued)` → `Ok(())`)
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/shared_message_queue/tests.rs` (`matches!(outcome, EnqueueOutcome::Enqueued)` → `outcome.is_ok()` 等)
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/balancing_dispatcher.rs` の `SharedMessageQueueBox` impl (戻り値型追従)

#### 全 queue 実装テストの追従

- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue/tests.rs` (`Ok(EnqueueOutcome::Enqueued)` 等の match arm を `Ok(())` に置換)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_control_aware_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_priority_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_stable_priority_message_queue/tests.rs`
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_deque_message_queue/tests.rs`

#### Mailbox 上位 API + dispatcher hook の追従

- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::enqueue_envelope` の戻り値を `Result<(), SendError>` に変更
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::enqueue_user` を新しい戻り値型に追従
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::prepend_user_messages` 内の `EnqueueOutcome` 分岐を削除 (`Result<(), SendError>` の `?` でフォールバック)
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs` の mock queue impl を新 trait シグネチャに追従
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher.rs::dispatch` default impl から `EnqueueOutcome::Pending` 分岐削除、`envelope_for_error = envelope.clone()` 削除、戻り値構築を `Ok(vec![mailbox])` に簡素化

- 検証: `cargo check -p fraktor-actor-core-rs --lib --tests` + `cargo test -p fraktor-actor-core-rs --lib`

### Commit 4: `refactor(executor): remove supports_blocking trait method and SpawnError::InvalidMailboxConfig`

- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs` から `supports_blocking()` trait method 削除 + rustdoc 整理
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/inline_executor.rs` から `supports_blocking()` impl 削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/inline_executor/tests.rs` から `supports_blocking_returns_false` テスト削除 + 他テストの `assert!(!executor.supports_blocking())` 行削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs` から `supports_blocking()` convenience method 削除
- 修正: `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared/tests.rs` から `supports_blocking_query` テスト + mock executor の `supports_blocking` impl 削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tokio_executor.rs` から `supports_blocking()` impl 削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tokio_executor/tests.rs` から `supports_blocking_returns_true` テスト削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/threaded_executor.rs` から `supports_blocking()` impl 削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/threaded_executor/tests.rs` から `supports_blocking_returns_true` テスト削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/pinned_executor.rs` から `supports_blocking()` impl 削除
- 修正: `modules/actor-adaptor-std/src/std/dispatch/dispatcher/pinned_executor/tests.rs` から `supports_blocking_returns_false` テスト削除
- 修正: `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs:880` の mock executor の `supports_blocking` impl 削除
- 修正: `modules/actor-core/src/core/kernel/system/base/tests.rs:154` の mock executor の `supports_blocking` impl 削除
- 修正: `modules/actor-core/src/core/kernel/actor/spawn/spawn_error.rs` の `SpawnError::InvalidMailboxConfig` variant + `invalid_mailbox_config()` constructor 削除 (caller ゼロを確認済み)
- 修正: `modules/actor-core/src/core/kernel/actor/actor_cell.rs:157` の `Returns SpawnError::InvalidMailboxConfig if ...` rustdoc 行削除
- 検証: `cargo check -p fraktor-actor-core-rs --lib --tests` + `cargo check -p fraktor-actor-adaptor-rs --lib --tests` + `cargo test -p fraktor-actor-core-rs --lib` + `cargo test -p fraktor-actor-adaptor-rs --lib`

### Commit 5: `docs(openspec): mark Block / DispatcherWaker capabilities as removed`

- 確認: `openspec/changes/mailbox-block-overflow-removal/specs/dispatch-executor-unification/spec.md` (既に作成済み、REMOVED + MODIFIED)
- 確認: `openspec/changes/mailbox-block-overflow-removal/specs/dispatcher-attach-detach-lifecycle/spec.md` (既に作成済み、MODIFIED)
- 検証: `openspec validate mailbox-block-overflow-removal --strict`

### 最終検証

- [ ] `./scripts/ci-check.sh ai all` exit 0
- [ ] `openspec validate mailbox-block-overflow-removal --strict` valid
- [ ] `grep -rn "MailboxOverflowStrategy::Block" modules/ showcases/` がヒット 0
- [ ] `grep -rn "MailboxOfferFuture" modules/ showcases/` がヒット 0
- [ ] `grep -rn "dispatcher_waker\|DispatcherWaker" modules/ showcases/` がヒット 0
- [ ] `grep -rn "EnqueueOutcome" modules/ showcases/` がヒット 0
- [ ] `grep -rn "supports_blocking" modules/ showcases/` がヒット 0
- [ ] `grep -rn "InvalidMailboxConfig\|invalid_mailbox_config" modules/ showcases/` がヒット 0

## Open Questions

- `dispatcher-pekko-1n-redesign` の archive はいつ実施されるか? Phase 14.5 / 14.5.1 を別 change に分離するか、本 change と並行して別途 follow-up change として archive 待ちにするかを確定する必要がある (現状ユーザ判断で「`dispatcher-pekko-1n-redesign` は archive せずに残す」方針なので、本 change の spec delta は post-archive baseline に対する書き方のままとする)
