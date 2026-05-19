# 設計メモ: pekko-restart-completion

## 参照 Pekko ソース

| ファイル | 行 | 対応 fraktor 項目 |
|---------|----|-------------------|
| `FaultHandling.scala` | 92-118 (`faultRecreate`) | AC-H4 `fault_recreate(cause)` |
| `FaultHandling.scala` | 278-303 (`finishRecreate`) | AC-H4 `finish_recreate(cause)` |
| `FaultHandling.scala` | 327-351 (`handleChildTerminated`) | AC-H4/H5 `handle_death_watch_notification` dispatch |
| `Children.scala` | 178-188 (`setChildrenTerminationReason`) | 既存 API (AC-H2 配線) |
| `Children.scala` | 240-257 (`handleChildTerminated`) | AC-H4/H5 state_change 消費 (`handle_death_watch_notification` 経由) |
| `DeathWatch.scala` | `watching` / `terminatedQueued` / `DeathWatchNotification` | AC-H5 実装 |
| `Actor.scala` | `preRestart(reason)` / `postRestart(reason)` | AL-H1 (trait 完了済、kernel 呼び出しのみ) |

## state machine

```
ActorCell
│
├── receive Recreate(cause)
│      ↓
│   fault_recreate(cause)
│      ├─ pre_restart(&mut ctx, &cause)
│      ├─ deferred_recreate_cause = Some(cause)
│      ├─ set_children_termination_reason(Recreation(cause)):
│      │    ├─ Normal 状態で子が居れば → true (defer)
│      │    └─ 子が居ない → false (immediate fallthrough)
│      ↓
│   [defer] children が 1 人ずつ死亡し DeathWatchNotification(pid) が到着
│           └─ handle_death_watch_notification(pid)
│               └─ remove_child_and_get_state_change(pid)
│                   → Some(Recreation(cause)) → finish_recreate(cause)
│
└── [immediate] finish_recreate(cause)
       ├─ drop_pipe_tasks / drop_stash_messages / ...
       ├─ publish_lifecycle(Stopped)
       ├─ recreate_actor
       ├─ clear_failed
       ├─ mailbox().resume()
       ├─ actor.post_restart(&mut ctx, &cause)
       │     └─ 成功 → publish_lifecycle(Restarted)
       │     └─ 失敗 → set_failed_fatally + report_failure
       └─ deferred_recreate_cause = None
```

## 親子 internal supervision watch 配線 (AC-H4 の前提)

現行 `spawn_with_parent` (`system/base.rs:605`) は `register_child` を呼ぶだけで watch は貼らない。
そのため通常の `spawn_child` では parent が child の停止通知を受けられず、
本 change で `SystemMessage::Terminated` を kernel から廃止すると AC-H4 の
`finish_recreate` が発火しなくなる。

### TOCTOU-safe な登録順序

Pekko の「親は暗黙に子を watch している」挙動に合わせ、`spawn_with_parent` で
`register_child` と internal watch 両サイド登録を **`perform_create_handshake` より前に** 完了させる:

```
spawn_with_parent(parent_pid, props)
  ├─ register_cell(child_cell)
  ├─ register_child(parent_pid, pid)        ← 既存 step 4 から前倒し:
  │       ← handle_death_watch_notification が
  │          remove_child_and_get_state_change(pid) で Some(state_change) を
  │          返せるようにするため
  ├─ INTERNAL WATCH（本 change で追加、Create handshake より前に実行）:
  │    ├─ child_cell.state.register_watcher(parent_pid, WatchKind::Supervision)
  │    │     ← stop 時に parent へ DeathWatchNotification を送るため
  │    └─ parent_cell.state.register_watching(pid, WatchKind::Supervision)
  │          ← parent が handle_death_watch_notification で watching チェックを通すため
  ├─ perform_create_handshake(parent, pid, &cell)   ← SystemMessage::Create 送信
  │       ├─ Ok → return ChildRef
  │       └─ Err → rollback_spawn:
  │                  ├─ parent_cell.state.unregister_watching(pid, WatchKind::Supervision)
  │                  ├─ unregister_child(parent_pid, pid)
  │                  └─ child cell は remove_cell で破棄（watchers は一緒に消える）
  └─ return ChildRef
```

上記順序により、child が `pre_start` で即座に失敗して停止するケースでも:

1. `notify_watchers_on_stop` は既に登録済みの `parent_pid` を参照でき、parent へ
   `DeathWatchNotification(child_pid)` を確実に配送できる
2. parent の `handle_death_watch_notification` が `remove_child_and_get_state_change(pid)` を
   呼んだとき、child が `children_state` に登録済みのため `Some(state_change)` が返り、
   `finish_recreate` / `finish_terminate` の dispatch が起動する

どちらの状態も Create 送信より前に確立されているため TOCTOU は存在しない。

### WatchKind による責務分離

user watch と internal supervision watch を同じ `Vec<Pid>` で管理すると、
user が `ctx.unwatch(child)` を呼んだときに supervision watch も解除され
AC-H4 が壊れる。これを防ぐため本 change で `WatchKind` enum を導入する。

```rust
pub(crate) enum WatchKind {
    User,        // ctx.watch / ctx.watch_with 由来
    Supervision, // spawn_with_parent 由来の親子 internal watch
}

pub(crate) struct ActorCellState {
    // Vec<Pid> → Vec<(Pid, WatchKind)> に変更
    watchers: Vec<(Pid, WatchKind)>,
    watching: Vec<(Pid, WatchKind)>,
    // ...
}
```

API セマンティクス:

| API | 操作 |
|-----|------|
| `spawn_with_parent` | `register_watcher(parent, Supervision)` + `register_watching(child, Supervision)` |
| `ctx.watch(target)` | `register_watching(target, User)` + target へ `SystemMessage::Watch` |
| `ctx.unwatch(target)` | `unregister_watching(target, User)` のみ除去、`Supervision` は保持 |
| `handle_watch(watcher)` | `register_watcher(watcher, User)` |
| `handle_unwatch(watcher)` | `unregister_watcher(watcher, User)` のみ除去、`Supervision` は保持 |
| `handle_death_watch_notification` | `watching_contains_pid(pid)`（kind 区別なし）で判定 |
| `notify_watchers_on_stop` | 全 `watchers` に kind 区別なく `DeathWatchNotification` 送信 |

`spawn_child_watched` (`actor_context.rs:345`) は `ctx.watch(child)` を追加呼び出しするため、
既に存在する `(child, Supervision)` に加えて `(child, User)` が併存する。これは冪等な
多重登録として扱われる。

## DeathWatchNotification routing (AC-H5)

`SystemMessage::DeathWatchNotification(Pid)` は kernel 内で Terminated を伝搬する唯一の envelope。
`on_terminated` は kernel が直接呼び、user queue 経由にしない（Run 3 plan B1-D 準拠）。

```
stopping cell (被 watch 側)
  └─ handle_stop 末尾 (既存 notify_watchers_on_stop を DeathWatchNotification に移行)
     └─ foreach watcher
          └─ system.send_system_message(watcher, DeathWatchNotification(self.pid))

即時通知の 2 経路も統一 (AC-H5 修正):
  - ActorCell::handle_watch (target 既終了)
       → send_system_message(watcher, DeathWatchNotification(self.pid))
  - ActorContext::watch (Err(SendError::Closed))
       → send_system_message(self.pid, DeathWatchNotification(target))

watcher cell  (state.watching / state.watchers は Vec<(Pid, WatchKind)> 型)
  └─ system_invoke: DeathWatchNotification(pid)
     ├─ if !state.watching_contains_pid(pid) → drop (User / Supervision どちらも未登録)
     ├─ if terminated_queued.contains(pid) → drop (dedup)
     └─ else:
        ├─ watching.retain(|(p, _kind)| p != pid)  (User / Supervision 両 entry を一括除去)
        │   + terminated_queued.push(pid)  (atomic)
        ├─ state_change = remove_child_and_get_state_change(pid)
        ├─ take_watch_with_message(pid):
        │    ├─ Some(msg) → actor_ref().try_tell(msg)  (user queue 経由、カスタムメッセージ)
        │    └─ None     → actor.on_terminated(&mut ctx, pid)  (kernel 直接呼び、user queue 非経由)
        ├─ terminated_queued.retain(|p| p != pid)  (dedup 保持期間 = push 〜 ここ まで)
        └─ match state_change:
             ├─ Some(Recreation(cause)) → finish_recreate(cause)
             ├─ Some(Creation/Termination) → TODO(Phase A3)
             └─ None → 何もしない
```

`SystemMessage::Terminated(Pid)` variant は本 change で **enum 定義から削除される**
（`system_invoke` の `Terminated(pid)` match arm も削除）。「後方互換を保つコードを書かない」
原則に従い、kernel 内で送信元が消えた未使用 variant を残さない。remote / cluster 経路で
将来必要になれば、その時点で該当 change で再導入する。

## CQS 違反（許容）

Pekko `set_children_termination_reason(reason) -> bool` と `remove_child_and_get_state_change(pid) -> Option<SuspendReason>` は `&mut self` + 戻り値の CQS 違反。Pekko の atomic update-and-observe セマンティクス保持のため分離不可能。`cqs-principle.md` の「Vec::pop 相当」例外に該当。doc コメントで `// NOTE: CQS exception — Pekko state machine requires atomic update+observe.` を明記する。

## 削除/非目標

- `SupervisorStrategy::handle_child_terminated` hook — Phase A3
- `faultCreate()` の `actor == null` 分岐 — 本 change は既存 actor instance の restart path のみ
- `finish_terminate` / `finish_create` dispatch — 本 change は `Recreation` 完了駆動のみ、`Termination` / `Creation` は `// TODO(Phase A3)` マーク
- `actor_cell.rs` (1419 行) のファイル分割 — 機能追加と構造変更を同一 change で混ぜない
