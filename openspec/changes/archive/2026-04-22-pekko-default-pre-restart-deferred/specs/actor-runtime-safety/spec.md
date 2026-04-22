## ADDED Requirements

### Requirement: ExecutorShared は drain owner を外部から宣言する guard API を提供しなければならない

`ExecutorShared` は、外部 caller が既存トランポリン機構の drain owner を宣言する API `enter_drive_guard(&self) -> DriveGuardToken` を提供しなければならない（MUST）。

戻り値の `DriveGuardToken` は RAII でライフサイクル管理され、`Drop` 実装が唯一の release 経路である（MUST）。`DriveGuardToken` には `#[must_use]` 属性が付与されなければならない（MUST、`let _ = enter_drive_guard()` のような即 drop 誤用をコンパイル時に検出するため）。公開 `exit_drive_guard` メソッドは **提供してはならない**（MUST NOT）。これは enter / release のペア違反（release 忘れ・二重 release）を型システムで防止するためである。

既存トランポリン機構は `running: ArcShared<AtomicBool>` + `trampoline: SharedLock<TrampolineState>` (`executor_shared.rs:40-146`) で構成される。`enter_drive_guard` は既存 `running` を `compare_exchange(false, true)` で claim する。
CAS が成功した場合 `DriveGuardToken { claimed: true }` を返し、drop 時に `running` を false に戻す。
CAS が失敗した場合（他の drain owner が既に active）は `DriveGuardToken { claimed: false }` を返し、
drop 時は何もしない（外側 owner の運用を尊重する）。

#### Scenario: enter_drive_guard は既存 running フラグを CAS で claim する

- **GIVEN** `ExecutorShared` の `running` が `false` である状態
- **WHEN** `enter_drive_guard()` が呼ばれる
- **THEN** `running` が `true` に遷移する（atomic CAS）
- **AND** 戻り値 `DriveGuardToken` の `claimed` フィールドが `true`

#### Scenario: drain 中の enter_drive_guard は no-op token を返す

- **GIVEN** 既存 `ExecutorShared::execute` が drain loop 実行中で `running = true` の状態
- **WHEN** 別の caller が `enter_drive_guard()` を呼ぶ
- **THEN** CAS が失敗し `running` は `true` のまま維持される
- **AND** 戻り値 `DriveGuardToken` の `claimed` フィールドが `false`

#### Scenario: DriveGuardToken drop は claimed=true のときのみ running を false にする

- **GIVEN** `enter_drive_guard` で `claimed = true` の token を受け取った状態（`running = true`）
- **WHEN** token が scope を抜けて drop される
- **THEN** `running` が `false` に戻る
- **AND** pending トランポリン queue は drain されない（残ったタスクは次の外部 `execute` 呼び出しで処理される）

#### Scenario: DriveGuardToken drop で claimed=false は何もしない

- **GIVEN** `enter_drive_guard` で `claimed = false` の token を受け取った状態（外側 drain owner が active）
- **WHEN** token が scope を抜けて drop される
- **THEN** `running` の状態は変化しない（外側 drain owner が継続運用）

#### Scenario: 公開 exit_drive_guard API は存在しない

- **WHEN** `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs` の public
  API を grep で確認する
- **THEN** `pub fn exit_drive_guard` は定義されていない
- **AND** release 経路は `DriveGuardToken::drop` のみ

### Requirement: DriveGuardToken::drop は tail drain を実行してはならない

`DriveGuardToken::drop` 実装は `running = false` への遷移以外のロジック（特に pending drain）を実行してはならない（MUST NOT）。

具体的には、既存 `ExecutorShared::execute` の Step 4 "tail drain" (`executor_shared.rs:109-132`) 相当の pending drain を `drop` 内でコピー実装してはならない。これは、tail drain を `drop` 時に実行すると、guard 中に積まれた child.Stop が guard 解除直後に同期 drain され、parent の `fault_recreate` スタック上で child mailbox が動く再入が発生するためである。この再入は Pekko async dispatcher では起こらない挙動であり、`al_h1_t2` テストの `mid_snapshot == ["pre_start", "post_stop"]` + `children_state_is_terminating()` 契約を破壊する。

pending task は `DriveGuardToken::drop` 後に誰かが `ExecutorShared::execute` を呼ぶまで trampoline queue に保持され、その時点で通常の drain owner 選出を経て処理される。

#### Scenario: DriveGuardToken::drop 内には pending drain ロジックが書かれていない

- **WHEN** `DriveGuardToken::drop` 実装を確認する
- **THEN** `running.store(false, Ordering::Release)` 以外のロジックが存在しない（`claimed=true` 分岐内）
- **AND** `trampoline.pending` / `with_write(|inner| inner.execute(...))` への参照が `drop` 実装内に
  存在しない

#### Scenario: guard 中に積まれた pending は token drop では drain されない

- **GIVEN** `enter_drive_guard` で `claimed=true` の token を取得し、guard 中に `execute(task)` が
  1 回以上呼ばれて `trampoline.pending` に task が積まれている状態
- **WHEN** token が scope を抜けて drop される
- **THEN** `task` は **実行されない**
- **AND** `trampoline.pending` に `task` が残ったまま
- **AND** `running` が `false` に戻る

### Requirement: guard 中の execute は既存トランポリンにより pending に積まれなければならない

`ExecutorShared::enter_drive_guard` で `claimed = true` の token を保持している間に `ExecutorShared::execute` が呼ばれた場合、task は `trampoline.pending` に push されるだけで inner executor に同期実行されてはならない（MUST NOT）。

これは既存 `ExecutorShared::execute` の CAS ロジック（`running` が true なら drain owner になれず pending push で return）により自然に実現される。本 change で新規ロジックを追加するのではなく、既存機構に外部駆動 API を追加するだけで契約を満たす。

#### Scenario: guard active 中の execute は pending に積まれる

- **GIVEN** `ExecutorShared::enter_drive_guard` で `running = true` が claimed されている状態
- **WHEN** `execute(task, affinity_key)` を呼び出す
- **THEN** `task` は `trampoline.pending` に push される
- **AND** `running.compare_exchange(false, true)` が失敗するため inner executor の
  `execute` は同期的に呼ばれない
- **AND** 呼び出しは `Ok(())` を返す

#### Scenario: guard 解除後の pending は次の外部 execute で drain される

- **GIVEN** `DriveGuardToken` drop により `running = false` に戻り、pending に 1 件以上の task が残存
  している状態
- **WHEN** 別の caller が `execute(new_task, affinity_key)` を呼ぶ
- **THEN** `running.compare_exchange(false, true)` が成功し drain owner となる
- **AND** `new_task` + 残存 pending が順次 inner executor で処理される

### Requirement: MessageDispatcherShared::run_with_drive_guard は RAII で guard のペアを保証する

`MessageDispatcherShared` は `run_with_drive_guard<F, R>(&self, f: F) -> R` を提供しなければならない（MUST）。

この helper は内部で `self.executor().enter_drive_guard()` を呼び、RAII token を scope 終了まで保持しつつ `f()` を実行する。`f()` が panic しても `DriveGuardToken::drop` が呼ばれ `running` が false に戻ることを保証しなければならない（MUST）。

#### Scenario: run_with_drive_guard は f 実行前に enter、後に token drop を行う

- **WHEN** `run_with_drive_guard(|| { /* f body */ })` が呼ばれる
- **THEN** `executor.enter_drive_guard()` が `f` 実行前に呼ばれる
- **AND** `f` が正常終了した場合、戻り値を返す前に `DriveGuardToken::drop` が scope 終了で呼ばれる
- **AND** `f` が panic した場合、panic unwind 中に `DriveGuardToken::drop` が呼ばれ `running` が
  `claimed=true` の場合は false に戻される

### Requirement: fault_recreate の pre_restart 呼び出しは reentrancy guard でラップされなければならない

`ActorCell::fault_recreate` は `Actor::pre_restart` 呼び出しを `MessageDispatcherShared::run_with_drive_guard` でラップしなければならない（MUST）。

ラップの範囲は `pre_restart` 呼び出し 1 点に限定され、`fault_recreate` 全体や `system_invoke` 全体、`finish_recreate` 内の `post_restart` 呼び出しには**適用してはならない**（MUST NOT）。これは

- default `pre_restart` のみが `stop_all_children` を呼ぶ唯一の Pekko 互換 lifecycle hook であり、
  他の system message 処理経路には同様の再入問題が存在しないこと
- ガード範囲を狭く保つことで、他の passing テスト（`handle_watch` / `handle_unwatch` / `handle_stop`
  / `handle_kill` / `handle_failure` 等を駆動するテスト）への副作用を避け、**新たな Pekko 非互換を
  生まないこと**

を保証するためである。

#### Scenario: fault_recreate 内の pre_restart はガードされる

- **WHEN** `ActorCell::fault_recreate` の実装を grep で検索する
- **THEN** `run_with_drive_guard` の呼び出しが `actor.pre_restart(&mut ctx, cause)` を囲む形で
  1 箇所だけ存在する
- **AND** ガードの外側に `set_children_termination_reason` 呼び出しと `finish_recreate` への
  fall-through ロジックが残っている

#### Scenario: fault_recreate 以外にはガードが適用されない

- **WHEN** `modules/actor-core/src/core/kernel/actor/` 配下で `run_with_drive_guard` の使用箇所を
  grep する
- **THEN** 呼び出し元は `fault_recreate` 関数内の 1 箇所のみである
- **AND** `system_invoke` 関数本体、`finish_recreate` 関数内、`handle_*` 系関数群には
  `run_with_drive_guard` が現れない

#### Scenario: Executor trait には変更がない

- **WHEN** `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs` を本 change の前後で
  diff する
- **THEN** trait 定義に変更がない（`enter_drive_guard` / `exit_drive_guard` 等の新メソッドが追加されて
  いない）
- **AND** `InlineExecutor` / `PinnedExecutor` / `ForkJoinExecutor` / `BalancingExecutor` などの
  `impl Executor for` 実装も変更されていない

#### Scenario: ガード適用によって既存の passing テストが Pekko 非互換を生じない

- **WHEN** `rtk cargo test -p fraktor-actor-core-rs` を実行する
- **THEN** `handle_watch` / `handle_unwatch` / `handle_stop` / `handle_kill` / `handle_failure` /
  `handle_suspend` / `handle_resume` / `handle_create` / `handle_death_watch_notification` を駆動する
  全テストが passing である
- **AND** 既存 `executor_shared/tests.rs` の全トランポリンテストが passing である
- **AND** 本 change で新規に `#[ignore]` が付与されたテストが存在しない
