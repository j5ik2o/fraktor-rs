## Context

現状の mailbox lifecycle は次のように ownership が分裂している。

```text
run()     : system/user dequeue と invoke を所有
detach()  : state.close() と queue clean_up を所有
```

この分裂のため、Phase II では `user_queue_lock` を使って次を同時に直列化した。

- `enqueue_envelope` / `prepend_user_messages_deque`
- `dequeue`
- `become_closed_and_clean_up`

これは close correctness を成立させる一方、`detach` が queue lane に直接介入する設計を固定してしまう。結果として `cleanup vs dequeue` 競合が state machine ではなく outer lock 依存で解決されている。

Phase III / 3.5 の完了により、prepend contract 自体は deque-only に固定された。したがって Phase IV 相当で最初に再設計すべき本体は lock 数ではなく **cleanup ownership** である。

## Goals / Non-Goals

**Goals**

- queue の terminal cleanup を exactly once で実行する owner を明確化する
- `detach` と in-flight `run()` の ownership 競合を state machine で解消する
- close request 後に新規 scheduling が始まらないことを維持する
- cleanup 後も既存の dead-letter / `LeaveSharedQueue` semantics を維持する
- 後続 change が outer lock 削減だけに集中できる前提を作る

**Non-Goals**

- この change 単体で `user_queue_lock` を全面撤去すること
- prepend batch atomicity の最終解法を入れること
- shared queue を含む dispatcher-level close semantics を再定義すること
- `MessageQueue` trait の全面 redesign

## Decisions

### 1. `detach` は direct cleanup owner ではなく close requester になる

`MessageDispatcherShared::detach` は mailbox queue を直接 drain / cleanup しない。代わりに mailbox に close request を立て、finalizer election を行う。

意図:

```text
detach
  -> request_close()
  -> try_acquire_finalizer()
     -> idle なら caller が finalize
     -> running なら runner に委譲
```

これにより dispatcher detach path は mailbox user lane の concurrent consumer ではなくなる。

### 2. terminal cleanup は finalizer が exactly once で実行する

cleanup の責務は次の 2 者のどちらか 1 つに限定する。

- idle mailbox を close した detach caller
- close request 中の mailbox を走り切る in-flight runner

このため schedule state は少なくとも概念的に次を区別できなければならない。

- close requested
- finalizer owned
- cleanup done

`closed` 1 bit だけでは「新規 schedule 禁止」と「cleanup 完了」が混ざるため不十分である。

### 2.1 `MailboxScheduleState` は 3 段階の terminal state を表現する

現行の `FLAG_CLOSED` は意味が粗すぎるため、少なくとも概念的には次の 3 段階に分割する。

- `CLOSE_REQUESTED`
  - 新規 `request_schedule()` を拒否する
  - まだ cleanup owner は未確定でもよい
- `FINALIZER_OWNED`
  - ある 1 主体が terminal cleanup を実行中または実行責任を保持している
  - 他の caller / runner は cleanup を重複実行してはならない
- `CLEANUP_DONE`
  - terminal cleanup が完了した
  - 以後 mailbox は観測専用の終端状態になる

ここで重要なのは bit 数ではなく役割分離である。`request_schedule()` が見るべき条件と、cleanup owner election が見るべき条件を分離することが目的であり、最終的な表現が bit field か enum-like encoding かは実装時に再判断してよい。

### 2.2 state helper の責務を command/query で分離する

state helper は command/query を分離しなければならない。実装前提として必要なのは、具体的な型名やメソッド名ではなく、次の責務分離である。

- scheduling 用 API
  - 新規 schedule を試みる
  - running への遷移を確定する
- run completion 用 API
  - running を外す
  - 通常 reschedule と terminal finalize を区別して caller へ返す
- close / cleanup ownership 用 API
  - close request を立てる
  - finalizer ownership を一意に獲得する
  - cleanup 完了を確定する
- query 用 API
  - running / suspended / close requested / cleanup done を個別に観測する

この設計で重要なのは次の 3 点である。

- `set_idle() -> bool` 相当の 2 値 API では terminal path を安全に表現できない
- close request と cleanup completion は別の state transition として扱う必要がある
- finalizer election の authoritative CAS は state object 内に閉じ込める必要がある

### 2.3 run completion は 2 値ではなく複数状態を返せる必要がある

現行の `set_idle() -> bool` は次を同時にやっている。

- `RUNNING` を落とす
- `need_reschedule` を消費する
- caller に「もう一度 schedule すべきか」を返す

これは通常 path では便利だが、terminal path を導入すると意味が曖昧になる。したがって run completion は 2 値ではなく、少なくとも次を区別できる複数状態の API に置き換える必要がある。

期待する意味論:

```text
- 通常 reschedule が必要
- 通常 idle で終了
- runner 自身が finalizer へ遷移すべき
- すでに terminal state なので通常 scheduling を再開しない
```

`bool` では「通常 idle」と「terminal path でその場終了」を区別できないため不十分である。

### 2.4 close request API は idle/running の分岐を call site へ漏らさない

`detach` 側に `is_running()` を読ませて if/else させるより、close request API 自体が coarse-grained な outcome を返すほうが誤用しにくい。

想定する意味論:

```text
close requester 自身が finalizer を取得した
  close request 成功。かつ idle なので caller が finalizer を持つ

runner が後で finalizer を取る
  close request 成功。runner が後で finalize する

すでに close request 済み
  すでに close request 済み。追加 action は不要

すでに cleanup 完了済み
  cleanup 完了済み。追加 action は不要
```

これにより detach path は次のように単純化できる。

```text
close request outcome を見て
  - caller finalizer なら即 finalize
  - それ以外なら return
```

### 2.5 finalizer election primitive は runner path で直接使える低レベル操作として残す

close request API が coarse outcome を返す一方で、runner path では close request 観測後に「いま自分が finalizer を取るべきか」を CAS で判断したい。

そのため low-level primitive を state object 側に残す。

使い分け:

- `detach` 側
  - coarse-grained な close request outcome を使う
- `run()` 側
  - close request 観測後に low-level finalizer election primitive を使う

この分離により detach と run が同じ low-level CAS protocol を直接なぞることを避け、call site ごとの誤実装を減らす。

### 2.6 query 名は `closed` ではなく phase 名を使う

この redesign 後に `is_closed()` という名前を `MailboxScheduleState` に残すと、close requested と cleanup done のどちらを意味するか曖昧になる。

したがって state helper では phase を明示した query 名を使う。

- `is_close_requested()`
- `is_cleanup_done()`
- `is_running()`
- `is_suspended()`

`Mailbox` public API の `is_closed()` は互換のため残してよいが、その実体がどの phase を指すかは `Mailbox` 側で明示的に決める。現段階では `close requested 以降を true` とするのが第一候補である。

### 3. running runner は close request を観測したら次の user dequeue を続けない

active runner が close request を観測したら、以後の責務は「通常 drain 継続」ではなく「terminal finalize」である。

守るべき意味論:

- すでに dequeue 済みの in-flight message は最大 1 件だけ通常処理され得る
- それ以降の queued user messages は actor へ delivery せず、cleanup policy に従って dead-letter / skip される

これにより現行の `become_closed_and_clean_up` が実質的に提供している「残キューは actor に渡さず cleanup する」契約を保つ。

### 3.1 close request 後に新たな system dequeue を増やさない

`system` queue についても、この change では close request 後に新しい drain 契約を導入しない。

判断:

- **許容するもの**
  - close request 観測前にすでに dequeue 済みだった 1 件の in-flight system message の完了
- **許容しないもの**
  - close request 観測後に次の system message を積極的に dequeue して通常処理すること

理由:

- 現行の `become_closed_and_clean_up` は user queue の cleanup しか規定しておらず、pending system messages を「detach 後も全部処理する」契約を持っていない
- ここで system queue まで drain 対象に広げると、termination / watch / resume 系の意味論まで同時に再設計することになり、scope が広がりすぎる
- ownership handoff change の主題は user lane の cleanup owner 統一であり、system lane の新 contract 導入ではない

したがって close request 後の `run()` は、現在処理中の 1 件を超えて system queue を前進させず、finalizer path へ移る。

### 4. idle path と running path の finalizer election を分ける

設計上の分岐は次のとおり。

```text
close request
   |
   +-- mailbox is idle
   |      -> detach caller acquires finalizer
   |      -> finalize immediately
   |
   +-- mailbox is running
          -> runner keeps current in-flight work
          -> runner acquires / confirms finalizer
          -> runner finalizes before leaving run()
```

この分岐により `RUNNING` の消失待ちを detach 側で block する必要がなくなる。inline executor / self-stop / nested scheduling と相性が良い。

### 4.1 遷移は `detach path` と `run path` の 2 本に限定する

状態遷移は次のように整理する。

```text
初期
  IDLE / SCHEDULED / RUNNING

detach:
  request_close()
    -> CLOSE_REQUESTED
    -> if !RUNNING && try_acquire_finalizer()
         -> FINALIZER_OWNED by detach caller
         -> finalize()
         -> CLEANUP_DONE
    -> else
         -> return (runner が引き取る)

run:
  set_running()
    -> RUNNING
  dequeue/invoke loop
    -> close request 観測
    -> stop normal user dequeue
    -> try_acquire_finalizer()
         -> FINALIZER_OWNED by runner
         -> finalize()
         -> CLEANUP_DONE
    -> set_idle() / return
```

ここで重要なのは、`detach` が `RUNNING` の消失待ちをしないことと、`run()` が `CLOSE_REQUESTED` 観測後に「再 schedule される通常 mailbox」として振る舞わないことである。

### 4.2 post-drain reschedule は close request を優先して失効する

現行の `register_for_execution` は `run()` の戻り値 `needs_reschedule` を見て再投入する。[message_dispatcher_shared.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs#L234)

ownership handoff 後は、close request が立った mailbox に対してこの post-drain reschedule が通常経路として再武装してはならない。

期待する意味論:

- `request_schedule()` は `CLOSE_REQUESTED` 以降を reject する
- `run()` は close request 観測後に `needs_reschedule = true` を返して通常再投入を促さない
- cleanup が未完でも、それは finalizer path の責務であり通常 scheduling の責務ではない

これにより「close request 後に通常 drain loop がもう 1 周始まる」パターンを排除する。

### 4.3 `run()` の戻り値は terminal path で通常 reschedule を返さない

現行の `run()` は `pending_reschedule || still_has_work` を返し、caller が通常 scheduling を再武装する。[base.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs#L181)

ownership handoff 後は、close request を観測した `run()` に対してこの契約をそのまま使ってはならない。

期待する意味論:

- 通常 path
  - 既存どおり `pending_reschedule || still_has_work`
- terminal path
  - finalizer を runner が取得して cleanup した場合: `false`
  - finalizer を detach caller が先に取得済みで、runner は close request を観測して終了するだけの場合: `false`

つまり close request を観測した `run()` は、「通常 mailbox として次の実行が必要か」を返すのではなく、「terminalize 済みなので通常 scheduling を再開しない」という結果を返す。

これにより `register_for_execution` 側は terminal path を特別扱いせずに済む。terminal mailbox が `true` を返してしまうと、`request_schedule()` 側で reject されるとしても、`needs_reschedule` の意味論が曖昧になるため採用しない。

### 5. `LeaveSharedQueue` は finalizer ownership の例外ではなく cleanup 動作の例外として扱う

`MailboxCleanupPolicy::LeaveSharedQueue` を持つ sharing mailbox でも、ownership handoff 自体は同じ state machine に従う。

違いは finalizer が実行する cleanup 内容のみである。

- 通常 mailbox: 残 user queue を drain し dead-letter 化し `clean_up()` する
- sharing mailbox: shared queue は drain せず、mailbox local な terminal cleanup のみ行う

これにより ownership model を policy ごとに分けずに済む。

### 6. この change は outer lock を「責務縮小」までに留め、全面撤去は次段に送る

本 change で解消するのは `cleanup vs dequeue` の ownership 競合であり、outer lock 由来の全責務ではない。

残る論点:

- producer close race (`enqueue_envelope` / `prepend_user_messages_deque`)
- prepend batch atomicity
- `user_len` / metrics snapshot の一貫性

したがって本 change は `remove-mailbox-outer-lock` の直接実装ではなく、その前提となる redesign と位置付ける。

### 6.1 この change 完了後に `user_queue_lock` に残る責務

cleanup ownership handoff 後も、`user_queue_lock` に残る可能性が高い責務は次の 3 つである。

- producer close race の authoritative re-check
- prepend batch atomicity
- metrics / `user_len` snapshot の一貫性

逆に、この change で削れる責務は次である。

- detach caller と runner の cleanup 競合
- cleanup と dequeue の ownership 競合

この切り分けを先に確定しておくことで、次段の outer lock reduction proposal は「どの責務を消し、どれを残すか」を曖昧にせずに済む。

## Risks / Trade-offs

- schedule state が複雑化する
- `run()` の終了条件に finalization 分岐が入るため、テストの観点が増える
- ownership handoff 後も producer path の outer lock は残るため、lock 数削減効果は限定的
- `LeaveSharedQueue` の semantics を壊すと BalancingDispatcher が regress する

## Open Questions

- finalization 完了後の metrics publish を誰が担当するか

解消済みの判断:

- finalizer election の authoritative CAS は `MailboxScheduleState` に閉じ込める
  - `Mailbox` 側 helper は orchestration のみを持つ
  - cleanup owner の一意性判定を `Mailbox` 側の複数メソッドへ分散させない
- close request 後に新しい system dequeue を増やさない
  - in-flight 済みの 1 件だけは完了し得る
  - 以後は finalizer path へ移る
- close request を観測した `run()` は terminal path で通常 reschedule を返さない
- 現状は `become_closed_and_clean_up()` の最後で cleanup 後 snapshot を publish している
  - ownership handoff 後も finalizer が metrics publish を担当するのが第一候補である
