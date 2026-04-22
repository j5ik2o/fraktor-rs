# 設計メモ: pekko-default-pre-restart-deferred

## 参照 Pekko ソース

| ファイル | 行 | 対応 fraktor 項目 |
|---------|----|-------------------|
| `Actor.scala` | 626-632 (default `preRestart`) | 既定 `Actor::pre_restart` が `stop_all_children` + `post_stop` を呼ぶ |
| `dungeon/Children.scala` | 129-142 (`stop(actor)`) | `shallDie(actor)` で container を Terminating 化し `actor.stop()` で enqueue |
| `dungeon/FaultHandling.scala` | 92-118 (`faultRecreate`) | `pre_restart` 後に `setChildrenTerminationReason(Recreation)` → 子が残れば defer |
| `dungeon/FaultHandling.scala` | 278-303 (`finishRecreate`) | 最後の child termination を契機に起動 |
| `dispatch/Mailbox.scala` | 228-238 (`Mailbox.run`) | 1 mailbox 単位の drain。**同一 thread 上で他 mailbox を再入 drain しない** |

## 既存 `ExecutorShared` トランポリンの確認

`modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs` (lines 40-146) は既に
**外側トランポリン機構**を実装している:

```rust
pub struct ExecutorShared {
    inner:      SharedLock<Box<dyn Executor>>,
    trampoline: SharedLock<TrampolineState>,          // pending タスクキュー
    running:    ArcShared<AtomicBool>,                // drain owner の排他
}

pub fn execute(&self, task: BoxedTask, affinity_key: u64) -> Result<(), ExecuteError> {
    // Phase 1: queue the task
    self.trampoline.with_lock(|state| state.pending.push_back(QueuedTask { task, affinity_key }));

    // Phase 2: become drain owner via CAS
    if self.running.compare_exchange(false, true, ...).is_err() {
        return Ok(());  // 誰かが既に drain 中 → pending に積んだだけで return
    }

    // Phase 3: drain loop
    loop {
        let next = self.trampoline.with_lock(|state| state.pending.pop_front());
        match next {
            Some(queued) => self.with_write(|inner| inner.execute(queued.task, queued.affinity_key)),
            None => break,
        }
    }
    self.running.store(false, Ordering::Release);
    // ...tail drain...
}
```

このトランポリンは production / sync 両経路で **既に効いている**:
- Production (ForkJoinExecutor / PinnedExecutor): worker thread 上で `ExecutorShared::execute` が呼ばれ、drain owner が確立。別 thread が送る task は pending に積まれて dain loop で処理される
- Sync (InlineExecutor): 同一 thread 上で `ExecutorShared::execute` が再入しても、既に drain owner が確立していれば pending に積むだけで戻る

## 観測されている破れ

`al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate`
(`modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs:1551`) が `#[ignore]`
の理由として記録されている状況:

```
parent.system_invoke(Recreate(cause))    ← ActorCellInvoker 直呼び経路
  │                                         ExecutorShared を経由していない、running=false のまま
  └─ fault_recreate(cause)
       └─ pre_restart default
            └─ stop_all_children
                 └─ for child in children:
                      ├─ mark_child_dying(child)   // Normal → Terminating{UserRequest,[child]}
                      ├─ unregister_watching(child) // WatchKind::User のみ除去
                      ├─ remove_watch_with(child)
                      └─ send_system_message(child, Stop)
                           └─ dispatcher.system_dispatch
                                └─ enqueue Stop + register_for_execution
                                     └─ child.request_schedule: IDLE → SCHEDULED
                                     └─ ExecutorShared::execute(child_run)
                                          ├─ trampoline.pending.push(child_run)
                                          ├─ running.CAS(false, true) → 成功 (誰も drain 中でない)
                                          ├─ drain loop 開始
                                          │   └─ pop child_run → with_write → InlineExecutor::execute(child_run)
                                          │        └─ child mailbox.run
                                          │             └─ handle_stop
                                          │                  └─ notify_watchers_on_stop
                                          │                       └─ send_system_message(parent, DeathWatchNotification)
                                          │                            └─ ExecutorShared::execute(parent_run)
                                          │                                 ├─ trampoline.pending.push(parent_run)
                                          │                                 └─ running.CAS(false, true) → 失敗 (drain 中)
                                          │                                 └─ return (drain owner に任せる)
                                          ├─ drain loop 次: pop parent_run → with_write → InlineExecutor::execute(parent_run)
                                          │   └─ parent mailbox.run
                                          │        └─ handle_death_watch_notification(child)
                                          │             └─ watching_contains_pid(child)=true (Supervision 残存)
                                          │             └─ remove_child_and_get_state_change(child)
                                          │                  // container: Terminating{UserRequest,[child]}
                                          │                  // to_die 空 → reason=UserRequest で return
                                          │             └─ state_change = Some(SuspendReason::UserRequest)
                                          │             // Recreation ではないので finish_recreate 不発
                                          └─ drain loop: pending 空 → break、running=false
            └─ post_stop (default)
       └─ set_children_termination_reason(Recreation(cause))
            // container は既に Normal/Empty (child は上記で removed)
            // → false を返す
       └─ deferred=false なので finish_recreate(cause) へ即時 fall-through
            └─ post_restart default → pre_start 連鎖が inline 実行
```

## 根本原因

**parent の `ActorCellInvoker::system_invoke` は `ExecutorShared::execute` を経由しない直呼び経路のため、`ExecutorShared::running` が false のまま `send_system_message(child, Stop)` による child 側の execute が最初の drain owner になり、drain loop の中で parent mailbox への DWN 配送も同期処理されてしまう。**

既存 `ExecutorShared` トランポリンは production では効いているが、test の直呼び経路では **誰も先に drain owner を確保していない** ため、child の execute が owner を奪い parent mailbox run まで drain してしまう。

## 採用する設計: `ExecutorShared::enter_drive_guard` / `exit_drive_guard` で drain owner を外部宣言

### 原則

- **既存の `ExecutorShared::running` AtomicBool + trampoline pending キューを流用する**。新たな機構を作らない
- **`Executor` trait には触らない**。guard は `ExecutorShared` レベルで完結する。production executor / InlineExecutor はいずれも変更不要
- **ガード適用範囲は `fault_recreate` 内の `pre_restart` 呼び出し 1 点に限定**
- **`enter_drive_guard` は既存 drain owner を尊重する**。CAS 失敗時は no-op で、外側 drain owner に任せる。これにより nested enter や production 並行経路との衝突が起きない
- **`DriveGuardToken::drop` は pending を drain しない** (意図的な設計判断)。`claimed=true` なら `running=false` に戻すだけ。pending は次の外部 `execute` 呼び出しまで残る。これは Pekko async dispatcher の「invocation 終了時に他 actor mailbox を synchronous に flush しない」原則と一致
  - **既存 `ExecutorShared::execute` の Step 4 tail drain (`executor_shared.rs:109-132`) との明示的な違い**:
    `execute()` 関数は drain 終了後に pending が溜まっていれば自力で tail drain し直すが、
    `DriveGuardToken::drop` は tail drain を実行しない。理由: tail drain を `drop` 時に実行すると、
    guard 中に積まれた child.Stop が guard 解除直後に同期 drain され、child mailbox が parent の
    fault_recreate スタック上で動いてしまう。これは Pekko async dispatcher では起こらない再入で、
    test の `mid_snapshot` 契約 (`children_state_is_terminating() == true`) も破綻する。
    `drop` で tail drain を追加してはならない (MUST NOT)

### Pekko 互換性の非回帰確認

本 change の変更は次の 2 箇所に限定される:

1. `ExecutorShared` に `enter_drive_guard` / `exit_drive_guard` + RAII token 追加
2. `ActorCell::fault_recreate` 内、`actor.pre_restart(&mut ctx, cause)` 呼び出し 1 行を
   `MessageDispatcherShared::run_with_drive_guard` でラップ

他の system message handler (`handle_stop` / `handle_kill` / `handle_watch` /
`handle_unwatch` / `handle_death_watch_notification` / `handle_failure` 等) には**一切触れない**。
`Executor` trait にも触れない。したがって以下の既存 passing テストは挙動変化を受けない:

- `ac_h5_t*`: watch / unwatch / DeathWatchNotification 経路 (handle_watch / handle_unwatch)
- `ac_h3_t*`: mailbox suspend / resume 経路 (handle_suspend / handle_resume)
- `ac_h4_t2 / ac_h4_t3`: override pre_restart での deferred 経路 (pre_restart が stop_all_children を呼ばない
  ため guard が有効でも挙動不変)
- AL-H1 T1 / T3: override pre_restart 系
- `handle_failure` / supervisor directive 経路
- `executor_shared/tests.rs` の既存トランポリンテスト（guard は既存機構の上書きでなく orthogonal な追加）

唯一挙動が変わるのは、**default pre_restart が `stop_all_children` を呼ぶ sync dispatch ケース**で、
そこでは guard により「fault_recreate が pre_restart を呼ぶ前に ExecutorShared の drain owner を確保する」
挙動になる。これは Pekko async dispatcher での挙動と一致する方向の変更であり、**Pekko 非互換を新たに
作り出さない**。

### API 追加

#### `ExecutorShared` への追加

```rust
impl ExecutorShared {
    /// Claims the drain-owner slot so that subsequent `execute` calls during
    /// the caller's guarded window simply enqueue into the trampoline and
    /// return without draining.
    ///
    /// If another drain owner is already active (CAS fails), returns a token
    /// with `claimed = false`; the outer owner continues to drain as normal.
    /// This no-op behaviour on contention keeps nested guards and production
    /// multi-thread access safe without additional synchronisation.
    ///
    /// The returned token holds the drain-owner slot until it is dropped.
    /// Callers MUST keep the token alive for the duration of the guarded
    /// region; `#[must_use]` enforces this at the type level so that
    /// `let _ = executor.enter_drive_guard();` (which would drop the token
    /// immediately) is rejected at compile time with a `must_use` warning
    /// promoted to an error by project-wide lint settings.
    pub fn enter_drive_guard(&self) -> DriveGuardToken {
        let claimed = self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        DriveGuardToken { claimed, running: self.running.clone() }
    }
}

#[must_use = "DriveGuardToken must be held for the full guarded region; \
              drop it at the end of the scope where `enter_drive_guard` was called"]
pub struct DriveGuardToken {
    claimed: bool,
    running: ArcShared<AtomicBool>,
}

impl Drop for DriveGuardToken {
    fn drop(&mut self) {
        if self.claimed {
            self.running.store(false, Ordering::Release);
        }
    }
}
```

**注意**: release 経路は **RAII `Drop` のみ**。外部から直接呼び出す `exit_drive_guard` 等の API は
公開しない。これにより enter / release のペア違反（release を忘れる、二重 release）が型システムで
防止される。加えて `#[must_use]` 属性により、token を捨てる誤用（`let _ = ...` 等）もコンパイル時に
検出できる。

#### `MessageDispatcherShared::run_with_drive_guard`

```rust
impl MessageDispatcherShared {
    /// Runs `f` on the calling thread while the underlying `ExecutorShared`
    /// has its drain-owner slot claimed. Any nested `execute` calls triggered
    /// by `f` will see `running = true` and enqueue into the trampoline
    /// instead of draining synchronously on the current stack.
    ///
    /// Pending tasks accumulated during the guarded window are NOT drained on
    /// exit — they remain in the `ExecutorShared` trampoline queue. This matches
    /// Pekko async dispatcher semantics where actor invocation exit does not
    /// implicitly flush other actors' mailboxes.
    ///
    /// Visibility is `pub(crate)`; external crates should not need this helper
    /// directly. `fault_recreate` is the sole caller within `actor-core`.
    pub(crate) fn run_with_drive_guard<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // 既存 `MessageDispatcherShared::executor(&self) -> ExecutorShared`
        // (`message_dispatcher_shared.rs:83`) を再利用。`ExecutorShared` は内部で
        // `ArcShared<SpinSyncMutex<...>>` を保持するので clone は cheap で良い。
        let executor = self.executor();
        let _token = executor.enter_drive_guard();
        f()
        // _token は scope 終了で drop され、claimed=true なら running=false に戻る
    }
}
```

**設計**: `DriveGuard<'a>` のように外部参照を保持するラッパー struct は不要。`DriveGuardToken` が
`running: ArcShared<AtomicBool>` を owned clone で持つため、ライフタイム制約もない。
`run_with_drive_guard` は単に `_token` を let binding で保持して `Drop` まで生かす。

### `ActorCell::fault_recreate` の局所ラップ

**ガード適用範囲は `pre_restart` 呼び出し 1 点のみ**に限定する:

```rust
pub(crate) fn fault_recreate(&self, cause: &ActorErrorReason) -> Result<(), ActorError> {
    if self.is_failed_fatally() { return Ok(()); }

    {
        let mut ctx = self.make_context();
        ctx.cancel_receive_timeout();
        // Pekko 互換: default `pre_restart` は `stop_all_children` を呼ぶ。この中で
        // 各 child へ `send_system_message(child, Stop)` が発行されるが、同期
        // dispatcher 環境では ActorCellInvoker::system_invoke 直呼び経路が
        // ExecutorShared::running を立てていないため、child の execute が drain owner
        // を奪い parent mailbox への DWN 配送まで同期処理されてしまう。
        // `run_with_drive_guard` で外側 drain owner を確保し、nested execute 呼び出しを
        // ExecutorShared 既存 trampoline で pending に積むだけにすることで、Pekko async
        // dispatcher と同一の挙動を再現する。
        let dispatcher = self.new_dispatcher_shared();
        let result = dispatcher.run_with_drive_guard(|| {
            self.actor.with_write(|actor| actor.pre_restart(&mut ctx, cause))
        });
        result?;
        ctx.clear_sender();
    }

    debug_assert!(
        self.mailbox().is_suspended(),
        "fault_recreate expects the mailbox to be suspended (AC-H3 precondition)"
    );

    let deferred = self.state.with_write(|state| {
        state.deferred_recreate_cause = Some(cause.clone());
        state.children_state.set_children_termination_reason(SuspendReason::Recreation(cause.clone()))
    });

    if deferred { return Ok(()); }

    self.finish_recreate(cause)
}
```

`finish_recreate` 側の `post_restart` 呼び出しには **guard を適用しない**。理由:

- default `post_restart` は `pre_start` を呼ぶだけで、通常 `pre_start` は child 群の
  `send_system_message` を連鎖させない（child spawn 時の Create handshake は `spawn_with_parent`
  経由で別途スケジュールされる）
- 仮に override `post_restart` が子 spawn を行った場合でも、それは新規に start する child であり
  restart state machine との再入はない
- scope を最小化することで「ガードが想定外の副作用を生む」可能性を最小化

### `Mailbox::run` 経路の扱い

production ルートの `Mailbox::run` は既に `ExecutorShared::execute` の drain loop 内側で動いており
`running=true` が確立されている。したがって `Mailbox::run` を経由する `system_invoke` では
そもそも再入問題が発生しない。本 change は `Mailbox::run` 経路には触れない。

`ActorCellInvoker::system_invoke` を直接呼ぶ test 経路でも、ガードは `fault_recreate` 内の
`pre_restart` 呼び出しだけに限定されているため、他の system message (`Create` / `Stop` /
`Watch` / `Unwatch` / `Suspend` / `Resume` / `Kill` / `StopChild` / `DeathWatchNotification` /
`PipeTask` / `Failure`) を処理する既存 test は一切影響を受けない。

## 決定ログ / 却下案

### 却下案 1: `ActorCellInvoker::system_invoke` 全体を `run_with_drive_guard` でラップ

**却下理由**:

1. 他の system message (`Stop` / `Watch` / `Unwatch` / `Kill` 等) も guard 下で処理されることになり、
   それらの handler が発行する `send_system_message` に対する drain 挙動が変化する
2. actor-core の actor_cell/tests.rs には 80 箇所超、actor-core 全体で 47 件以上の `system_invoke`
   テスト呼び出しがあり、その多くが「cross-cell 操作が inline で処理される」前提で assert している
3. 局所化された問題 (default pre_restart + stop_all_children の再入) を解決するのに、テスト経路全体の
   セマンティクスを変えるのは副作用が大きすぎる
4. **新たな Pekko 非互換を作り出すリスクを排除**するため、ガード範囲は必要最小限に絞る

### 却下案 2: `stop_all_children` を「mark + enqueue only」へ分離

`stop_all_children` 内で `send_system_message(child, Stop)` を呼ばず、parent の post-turn
action queue に詰めておき、`fault_recreate` の末尾で flush する案。

**却下理由**:

1. child を止める責務が `stop_all_children` から `fault_recreate` へ漏れるため責務境界が曖昧になる
2. Termination / finishCreate 経路でも同様の問題が起きるが、それぞれ別の post-turn queue を用意するか or 統一 queue を作るかで複雑度が増す
3. Pekko の `stop(actor)` も `actor.stop()` で即時 enqueue しており、責務分離ではなく
   dispatcher 側の turn 境界で解決されている。本質的な根本治療は dispatch 側にある

### 却下案 3: `fault_recreate` の先頭で container を `Terminating{Recreation(cause)}` に pre-promote

`set_children_termination_reason` を `pre_restart` の前に呼ぶ案。

**却下理由**:

1. 現 `set_children_termination_reason` は container が既に `Terminating` の時だけ成功する。
   `Normal` から直接 `Terminating{Recreation}` へ遷移する新 API が必要
2. inline 経路で `handle_death_watch_notification` が先に走っても state_change が
   `Some(Recreation(cause))` を返してしまい、`finish_recreate` が inline で起動する。
   テスト期待値 `mid_snapshot == ["pre_start", "post_stop"]` を満たせない
3. Pekko は `preRestart` → `setChildrenTerminationReason` の順序で、先に reason を書かない
   → Pekko parity を崩す

### 却下案 4: `Executor` trait に `enter_drive_guard` / `exit_drive_guard` を追加し `InlineExecutor` が override する

**却下理由** (5 ラウンド目レビューで判明):

1. `ExecutorShared` (`executor_shared.rs:40-146`) が既に `trampoline: SharedLock<TrampolineState>` +
   `running: ArcShared<AtomicBool>` を使った外側トランポリンを実装している。この機構は production /
   sync どちらの経路でも drain owner を CAS で排他制御している
2. 問題は「`Executor` trait への guard 追加で解決できる」のではなく、「`ActorCellInvoker::system_invoke`
   直呼び経路が `ExecutorShared` を経由しないため既存トランポリンに参加できない」こと
3. したがって正しい介入点は `ExecutorShared` レベル。`InlineExecutor` や他の production executor を
   override する必要は一切ない
4. `Executor` trait に変更を加えると trait 実装者への潜在的影響が増える。`ExecutorShared` レベルで
   完結する方が変更範囲が小さく、既存アーキテクチャと整合する

### 却下案 5: guard exit で pending を drain する

`DriveGuardToken::drop` の時点で pending 全件を drain する案。

**却下理由**:

1. テスト `al_h1_t2` は `system_invoke(Recreate)` 戻り直後の中間 snapshot で
   children_state が Terminating であることを assert する。exit 時 drain だと、child の
   Stop が drain され DWN が parent に届き finish_recreate が inline 起動、children_state が
   Normal/Empty になって assert 失敗する
2. Pekko の async dispatcher は actor invocation 終了時に他 actor の mailbox を synchronous
   に flush しない。exit 時 drain は async dispatcher の挙動と乖離する
3. pending 残留は sync dispatcher のテスト artifact としては許容できる。test は
   `handle_death_watch_notification` を明示的に呼んで「DWN が届いた状態」を simulate するため、
   child mailbox の実際の drain は不要

### 採用案の利点

- **既存 `ExecutorShared` トランポリン機構を 1 API 追加だけで拡張**、新機構を作らない
- production dispatcher (`PinnedExecutor` / `ForkJoinExecutor` / `BalancingDispatcher`) /
  `InlineExecutor` いずれも **変更不要**
- `Executor` trait に変更なし → trait 実装者への影響ゼロ
- ガード範囲が `pre_restart` 呼び出し 1 点に局所化されているため、他の system message 経路を
  使うテストへの影響が**ゼロであることが型レベルで明らか**
- 責務境界が明確: 「default pre_restart が stop_all_children を呼ぶ sync dispatch 経路でのみ
  外側 drain owner を確保する」という不変条件が `ExecutorShared::enter_drive_guard` + `fault_recreate`
  の 2 箇所に閉じる
- **新たな Pekko 非互換を作らない**: guard が有効な状況 (sync dispatch + default pre_restart)
  の挙動は Pekko async dispatcher と一致する方向の変更のみ
- RAII `DriveGuardToken` により enter / exit のペア違反を型システムで防止

## state machine

本 change で state machine そのものは変化しない（`fault_recreate` / `finish_recreate` の
2 フェーズ構造は維持）。変化するのは **`SystemMessage::Stop` が child mailbox に届いた
後、その drain が同一 thread 上でいつ走るか** のスケジューリング境界のみ。

```
parent.system_invoke(Recreate(cause))
  ├─ fault_recreate(cause)
  │    ├─ pre_restart 呼び出し ― ここから run_with_drive_guard
  │    │    ├─ executor.enter_drive_guard()  → DriveGuardToken { claimed=true }
  │    │    │    // running.CAS(false, true) 成功。以後の execute は pending push で return
  │    │    ├─ actor.pre_restart(ctx, cause) — default 実装:
  │    │    │    ├─ ctx.stop_all_children()
  │    │    │    │    └─ for child:
  │    │    │    │         ├─ mark_child_dying(child)   // Normal → Terminating{UserRequest,[child]}
  │    │    │    │         ├─ unregister_watching(child)
  │    │    │    │         ├─ remove_watch_with(child)
  │    │    │    │         └─ send_system_message(child, Stop)
  │    │    │    │              └─ dispatcher.system_dispatch
  │    │    │    │                   └─ enqueue Stop + register_for_execution
  │    │    │    │                        └─ request_schedule: IDLE → SCHEDULED
  │    │    │    │                        └─ ExecutorShared::execute(child_run)
  │    │    │    │                             ├─ trampoline.pending.push(child_run)
  │    │    │    │                             └─ running.CAS(false, true) → 失敗 (guard 保有中)
  │    │    │    │                             └─ return (drain しない)
  │    │    │    └─ self.post_stop(ctx)  — parent の post_stop (log: "post_stop")
  │    │    └─ DriveGuardToken::drop
  │    │         └─ running.store(false)、pending は残置
  │    ├─ set_children_termination_reason(Recreation(cause))
  │    │    // container: Terminating{UserRequest,[child]} → Terminating{Recreation(cause),[child]}
  │    │    // returns true → deferred
  │    └─ return Ok(()) (deferred)
  └─ parent.system_invoke returns
      // ExecutorShared.trampoline には child_run が残留しているが drain されない
      // この時点で log = ["pre_start", "post_stop"]、children_state = Terminating{Recreation}

  [test explicitly calls] parent.handle_death_watch_notification(child.pid())
       ├─ watching_contains_pid(child) = true (Supervision 残存)
       ├─ terminated_queued.push(child)
       ├─ remove_child_and_get_state_change(child)
       │    // container: Terminating{Recreation(cause),[child]}
       │    // to_die 空 → reason=Recreation(cause) を返す
       ├─ has_user_watch = false (stop_all_children で User unregister 済)
       ├─ terminated_queued.retain (remove)
       └─ finish_recreate(cause)
            ├─ drop_pipe_tasks / drop_stash_messages / drop_timer_handles / drop_watch_with_messages
            ├─ publish_lifecycle(Stopped)
            ├─ recreate_actor — 新 actor instance 生成
            ├─ clear_failed
            ├─ mailbox.resume
            └─ actor.post_restart(ctx, cause) — default:
                 └─ self.pre_start(ctx)  — 新 actor の pre_start (log: "pre_start")
```

本 change の guard により、`system_invoke(Recreate)` が返った直後の中間 snapshot
(`mid_snapshot`) では child は container 上で live（Terminating{Recreation} の to_die）
として残っており、`children_state_is_terminating() == true` が成立する。

production の async dispatcher 上では、parent の `mailbox.run` は別 worker thread に投入された
task closure として実行される。`ExecutorShared::execute` 関数自体は内部 executor への task 引き渡し
が済めばすぐに return するため、parent.mailbox.run 実行中の worker thread は `ExecutorShared::execute`
の drain loop 内側にはおらず、`ExecutorShared::running` は通常 false である。したがって fault_recreate
内で `enter_drive_guard` が呼ばれた時点でも **CAS が成功して `claimed=true` となる** (production /
test 同じ)。

production 固有の挙動:
1. guard 保有中の `send_system_message(child, Stop)` は trampoline.pending に push、CAS 失敗で即 return
2. `DriveGuardToken::drop` で `running=false` に戻ると、child.Stop task は trampoline queue に残存
3. 直後にまた `ExecutorShared::execute` が (他 thread や後続の send から) 呼ばれたとき、そいつが drain
   owner を CAS 取得して pending を drain → 内部 executor 経由で別 worker thread に投入
4. 別 worker thread 上で child.mailbox.run が実行される → child 停止 → parent へ DWN 送信
5. 別 thread / 別 turn で parent の `handle_death_watch_notification` → `finish_recreate` が駆動される

production / test どちらの経路でも観測できる state 遷移の順序は一致し、timing (child.Stop が実際に
drain されるタイミング) のみが thread boundary の有無で異なる:
- test (sync): test が明示的に `handle_death_watch_notification` を呼ぶまで child.Stop は pending に残る
- production (async): guard 解除後の後続 execute で child.Stop が別 thread に投入され自然に実行される

## Pekko 参照実装との対応表

| fraktor 変更点 | Pekko 対応箇所 | 意味論的合致 |
|----------------|---------------|--------------|
| `ExecutorShared::enter_drive_guard` / `DriveGuardToken` の Drop-based release | `dispatch/Mailbox.scala` の 1 actor 単位 drain 保証 | production dispatcher では既存トランポリンが自然に保証、guard は test 直呼び経路で同じ不変条件を確立 |
| `fault_recreate` 内の `pre_restart` を guard でラップ | `dungeon/FaultHandling.scala:92-118` | Pekko は thread boundary で自然に deferred、fraktor は guard で明示化 |
| `DriveGuardToken::drop` で pending を drain しない | Pekko async dispatcher の「invocation exit で他 actor mailbox を flush しない」原則 | 一致 |
| ガード適用を `pre_restart` 1 点に限定 | Pekko `preRestart` の default が唯一 `stop_all_children` を呼ぶ lifecycle hook | 対応範囲最小 |

## CQS 違反の扱い

本 change では CQS 違反は新規に発生しない。`ExecutorShared::enter_drive_guard` は command
（状態変更）+ token を返す。CQS では本来 command は戻り値なしだが、RAII token はコマンドの
「解除手段をクライアントに渡す」ために必要な型。既存の `Vec::pop` / Builder パターン相当の
例外に近い。ただし本 change はこの token を単に `_token` で scope-binding するだけで
query として利用しないため、CQS 原則の精神には違反しない。

## TOCTOU 警戒

- `enter_drive_guard` の CAS 操作は atomic。競合する execute 呼び出しとは既存 `ExecutorShared::execute`
  の CAS ロジックと同じ排他関係で安全
- `DriveGuardToken::drop` は `running.store(false)` のみ。pending 側の操作はないため、
  concurrent execute 呼び出しが drain owner を奪取することが可能（意図通り）
- `enter_drive_guard` が CAS 失敗した token (`claimed=false`) は `Drop` で何もしないため、
  二重 release の可能性なし
- production の複数 worker thread 並行経路: thread A が drain 中のとき thread B が
  `enter_drive_guard` を呼ぶと、B は CAS 失敗で `claimed=false`。B の `Drop` は何もしない。
  A の drain 終了後 B が `f()` 内で execute した task は普通に trampoline 経由で処理される

## 削除 / 非目標

- `stop_all_children` の責務分離（mark / enqueue / execute）— 本 change では `ExecutorShared`
  既存トランポリン + 外部 guard API で十分と判断。follow-up で必要になれば別 change で
- `finish_terminate` / `finish_create` の deferred 化 — Phase A3
- typed 層 `Behavior::pre_restart` / `post_restart` への reason 引数追加 — Phase A3
- `ActorCellInvoker::system_invoke` 全体のラップ — Pekko 非互換を作らないために局所化
- `finish_recreate` の `post_restart` への guard 追加 — 現状の default `post_restart` は
  `pre_start` に委譲するのみで cross-cell `send_system_message` を発行しないため不要
- `Executor` trait の拡張 — `ExecutorShared` レベルで完結するため不要（5 ラウンド目レビューで判明した
  既存トランポリン機構の活用による設計簡素化）
