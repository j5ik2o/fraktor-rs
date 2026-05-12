# mailbox / dispatcher / async 化 判断材料

更新日: 2026-04-27

実装反映: 2026-05-12

## 目的

`actor` モジュール全体ではなく、`mailbox` と `dispatcher` を責務単位で深掘りし、

1. Pekko 互換性が部分対応に留まっている理由は何か
2. その理由は妥当か
3. `async` 化をどの層で受けるべきか

を判断する材料をまとめる。

この文書は実装方針そのものではなく、**設計判断の前提整理** を目的とする。

## 固定前提

この版では、次の前提を固定する。

- **std 環境では Tokio を使う**
- **embedded / no_std + async 環境では Embassy を使う**
- `actor-core` は引き続き no_std / alloc 中心の kernel として保ち、Tokio / Embassy への接続は adaptor 層で受ける
- 正式リリース前なので、後方互換のために現在の executor 名や既定動作へ固執しない

この前提では、以前より async 化の価値が高くなる。  
特に、Pekko の blocking bounded mailbox (`pushTimeOut`) を厚く再現するより、**Tokio / Embassy の task / timer / waker を活かせる executor adapter と、Pekko `pipeToSelf` 型の future-to-message adapter を整える** ほうが投資対効果が高い。

## 結論

短く言うと、固定前提後の推奨戦略は次である。

- **mailbox の drain loop は sync / non-awaiting のまま保つ**
- **default executor は async runtime の task 実行へ寄せる**
- **blocking workload は `Blocking` dispatcher / blocking executor へ明示的に隔離する**
- **actor-facing API は同期のまま保ち、async I/O は untyped kernel の `pipe_to_self` / `pipe_to` で message 化する**
- **typed API は kernel contract の薄い wrapper として整える**

この方針だと、mailbox の「部分対応」は単なる不足ではなく、**blocking mailbox 互換へ寄せすぎないための意図的な余白** と解釈できる。  
ただし前回版より判断が変わる点がある。`std=tokio` / `embedded=embassy` を固定するなら、`async` は edge helper に留めるだけではもったいない。一方で、Pekko 互換性を考えると actor handler 自体を async 化するのではなく、`pipe_to_self` / `pipe_to` を async adapter 境界として維持するのが筋である。

2026-05-12 時点で、未解決だった論点は `async-first-actor-adapters` change として次の API に反映した。

- std Tokio helper は `fraktor_actor_adaptor_std_rs::actor::tokio_actor_system_config` とし、default dispatcher に `TokioTaskExecutorFactory`、blocking dispatcher に既存 `TokioExecutorFactory` を登録する。
- 既存 `TokioExecutor` / `TokioExecutorFactory` は `spawn_blocking` 互換 executor として維持し、default task executor は追加 API の `TokioTaskExecutor` / `TokioTaskExecutorFactory` として分離する。
- embedded / no_std + async 環境向け adapter は `fraktor-actor-adaptor-embassy-rs` crate に置き、`EmbassyExecutorFactory`、`EmbassyExecutorDriver`、`EmbassyTickDriver`、`embassy_monotonic_mailbox_clock` を提供する。
- `TickDriverKind::Embassy` を追加し、Embassy 固有依存は `actor-core-kernel` へ入れない。
- `ActorContext::pipe_to_self` / `pipe_to` と `TypedActorContext::pipe_to_self` / `pipe_to` は、呼び出し側に `.await` を要求しない同期 API のまま、future completion を message に戻す canonical surface として rustdoc と tests で固定する。

現時点の推奨は、**full async core ではなく async-first adapter strategy** である。  
つまり `Actor::receive` / `TypedActor::receive` / `MessageInvoker::invoke` / `Mailbox::run` を `async fn` 化するのではなく、runtime adapter と untyped-first の future-to-message adapter surface を厚くする。

## 参照した主な実装

### fraktor-rs

- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs`
- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tokio_executor.rs`
- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/threaded_executor.rs`
- `modules/actor-core/src/core/kernel/actor/actor_lifecycle.rs`
- `modules/actor-core/src/core/kernel/actor/context_pipe/task.rs`
- `modules/actor-core/src/core/kernel/actor/actor_context.rs`
- `modules/actor-core/src/core/typed/actor/actor_context.rs`
- `modules/actor-core/src/core/typed/actor/typed_actor.rs`
- `modules/actor-core/src/core/typed/dsl/behaviors.rs`
- `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_runner.rs`
- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_executor_signal.rs`
- `modules/actor-adaptor-std/src/std/tick_driver/tokio_tick_driver.rs`
- `docs/gap-analysis/actor-mailbox-gap-analysis.md`
- `docs/plan/lock-strategy-analysis.md`

### Pekko

- `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala`
- `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/javadsl/ActorContext.scala`
- `references/pekko/docs/src/test/scala/docs/actor/typed/SharedMutableStateDocSpec.scala`
- `references/pekko/actor-typed-tests/src/test/java/jdocs/org/apache/pekko/typed/InteractionPatternsTest.java`

- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailboxes.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/AbstractDispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/BalancingDispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/PinnedDispatcher.scala`
- `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala`

## 1. mailbox の部分対応には理由があるか

### 1.1 ある。特に blocking bounded mailbox を避けている

Pekko の bounded mailbox 群は `pushTimeOut` を持つ。

- `BoundedMailbox`
- `BoundedPriorityMailbox`
- `BoundedStablePriorityMailbox`
- `BoundedDequeBasedMailbox`
- `BoundedControlAwareMailbox`

Pekko 側では、bounded queue が満杯になったときに block / timeout を使う設計が存在する。  
`Mailboxes.scala` も non-zero `pushTimeOut` に対して warning を出す。

一方、fraktor-rs の bounded mailbox は `pushTimeOut` ではなく次の overflow strategy に寄っている。

- `DropNewest`
- `DropOldest`
- `Grow`

つまり、fraktor-rs は mailbox overflow を

- 待つ
- block する
- 条件変数で空きを待つ

ではなく、

- reject する
- evict する
- 観測可能な dead letter に流す

で扱う設計である。

### 1.2 この理由は mailbox を hot path と見ているから

`Mailbox::enqueue_envelope` は mailbox の最前線であり、高頻度で呼ばれる。  
現在の実装は overflow も dead letter 観測も mailbox 層で閉じるが、enqueue 自体は sync / non-blocking 前提で整理されている。

この設計は、Pekko の「bounded で待てる mailbox」を再現していない代わりに、
**mailbox の hot path に blocking を持ち込まない** 方向へ寄っている。

### 1.3 妥当性

これは十分妥当である。理由は次のとおり。

- actor runtime の基盤 hot path に blocking を入れると、executor / runtime 側の並列度や park/unpark の影響を強く受ける
- no_std / std 分離を重視している現在の構造では、blocking semantics を core へ入れると std 依存や runtime 依存が増える
- dead letter に観測できる overflow は運用上の説明責任を満たしやすい

したがって、**Pekko 完全互換ではないが、設計としては筋が通っている**。

## 2. dispatcher を深掘りすると何が見えるか

### 2.1 現在の dispatcher は「mailbox を実行する sync scheduler」である

`MessageDispatcherShared::register_for_execution` は、mailbox を scheduled にし、
`Executor` へ `Box<dyn FnOnce()>` を投げる。  
そこで実行されるのは `mailbox.run(throughput, throughput_deadline)` である。

重要なのはここで、dispatcher は

- `Future` を poll していない
- async task を直接扱っていない
- mailbox の drain loop を closure として実行している

つまり current model は **future-driven dispatcher ではなく mailbox-driven dispatcher** である。

### 2.2 Pekko との一致点

この形は Pekko にかなり近い。

- Pekko `Dispatcher.dispatch` も mailbox に enqueue して `registerForExecution` する
- Pekko `registerForExecution` も mailbox を executor へ submit する
- `Mailbox.run()` が system message → user message の順に drain する
- `BalancingDispatcher` は共有 queue を mailbox 群で drain する

したがって、dispatcher core の発想は「async runtime の executor」より **Pekko 型の mailbox scheduler** である。

### 2.3 fraktor-rs 独自の差分

差分として重要なのは次である。

1. `MessageDispatcherShared` が orchestration を外側に持っている
2. `dispatch` hook は mailbox 候補配列を返し、lock を離してから scheduling する
3. `ExecutorShared` が trampoline を持ち、inline 実行や再入 submit を吸収する

これは Rust 側の lock / ownership / reentrancy 制約に合わせた設計であり、
**async 化のための布石というより、sync closure 実行を安全に回すための設計** である。

## 3. blocking workload はどこで受ける設計か

### 3.1 mailbox ではなく dispatcher / executor 側で受ける設計が見える

typed 側には `DispatcherSelector::Blocking` がある。  
typed dispatcher 層はこれを `pekko.actor.default-blocking-io-dispatcher` へ解決する。

std adaptor 側でも:

- `TokioExecutor` は `spawn_blocking` を使う
- `ThreadedExecutor` は 1 task ごとに OS thread を切る blocking 向け executor である

つまり、現行アーキテクチャの意図はかなり明確で、

**block する可能性が高い処理は mailbox に持ち込まず、dispatcher / executor の選択で隔離する**

となっている。

### 3.2 この方針の利点

- mailbox semantics を単純に保てる
- bounded overflow と block を混ぜずに済む
- no_std core と std adaptor の境界を保ちやすい
- Tokio 環境でも、少なくとも blocking task は worker 直上ではなく blocking pool 側へ逃がせる

### 3.3 この方針の弱点

- Pekko の mailbox tuning をそのまま持ってくると違和感が出る
- 「bounded mailbox が満杯なら一定時間待つ」という運用設計はそのまま移植できない
- mailbox 選択だけで backpressure を調整する思想は薄くなる

## 4. 既に存在する async 接続点

### 4.1 actor 自体は sync contract

`Actor` trait の中核は sync である。

- `pre_start(&mut self, ...) -> Result<...>`
- `receive(&mut self, ...) -> Result<...>`
- `post_stop(&mut self, ...) -> Result<...>`

`MessageInvoker` も sync であり、

- `invoke(&mut self, message: AnyMessage) -> Result<...>`
- `system_invoke(&mut self, message: SystemMessage) -> Result<...>`

になっている。

これは非常に重要で、今の runtime は **actor invocation そのものを async にしていない**。

### 4.2 その代わり `pipe_to_self` / `pipe_to` がある

一方で `ActorContext` には async future を mailbox へ橋渡しする seam がある。

- `pipe_to_self`
- `pipe_to`
- `ask` の結果を future として受けて再投入する経路

内部では `ContextPipeTask` が `Pin<Box<dyn Future<...>>>` を保持し、
waker により再度 mailbox 側へ戻ってくる。

つまり現在の async story は:

- actor handler は sync
- 非同期 I/O や future は actor 外で進む
- 完了時に message として mailbox へ戻す

である。

これは Pekko typed の `pipeToSelf` 的な発想と整合している。

重要なのは、この seam は typed convenience ではなく **untyped kernel contract** として先に存在している点である。  
typed `TypedActorContext::pipe_to_self` / `pipe_to` は、kernel の future polling、waker、actor cell delivery、failure observation に乗る薄い wrapper として扱う。

## 5. 固定前提で戦略がどう変わるか

### 5.1 `spawn_blocking` は既定 executor として重くなる

現状の `TokioExecutor` は `Handle::spawn_blocking` で mailbox drain closure を実行する。  
これは「actor handler が blocking し得る」という保守的前提では安全側だが、`std=tokio` を固定し async-first に寄せるなら既定としては重い。

Tokio 前提では、executor は少なくとも次の 2 系統へ分けたほうがよい。

| dispatcher intent | executor | 想定用途 |
|---|---|---|
| default / non-blocking | Tokio task executor | mailbox drain が短く、handler が blocking I/O をしない actor |
| blocking | Tokio blocking executor | 同期ファイル I/O、CPU heavy、既存 sync API 呼び出し |

後方互換は不要なので、現在の `TokioExecutor` 名を残すことにこだわらず、既定側と blocking 側を責務で分けるほうがよい。  
例えば設計上は `TokioTaskExecutor` と `TokioBlockingExecutor` のように分け、`DispatcherSelector::Blocking` は後者へ解決する。

### 5.2 Embassy では blocking mailbox 互換の価値がさらに下がる

Embassy 前提では、`pushTimeOut` 的な blocking bounded mailbox は環境に合わない。  
embedded async では「満杯なら待つ」を thread block として実装できないし、executor 上で待つなら `await` 可能な backpressure protocol として設計する必要がある。

したがって、embedded 側の戦略は次が自然である。

- mailbox overflow は現行の `DropNewest` / `DropOldest` / `Grow` と観測可能な dead letter を維持する
- backpressure は bounded mailbox の blocking put ではなく、typed delivery / ask / pull protocol / mailbox pressure notification で表現する
- executor adapter は Embassy の static task / signal / bounded ready queue を使い、mailbox drain closure を短時間だけ実行する

### 5.3 scheduler には async 化の入口が既にある

`SchedulerRunner` には `AsyncHost` / `Hardware` のモード概念があり、`TickExecutorSignal` は `wait_async()` を持っている。  
`TokioTickDriver` も `tokio::time::interval` と async task で tick を供給している。

これは、actor invocation より先に **runtime driver を async 化する余地** が既にあることを示している。  
つまり最初に async 化すべき対象は `Actor::receive` ではなく、Tokio / Embassy の task・timer・waker を actor runtime に接続する adapter 群である。

### 5.4 async を入れてよい境界と入れない境界

固定前提でも、`Mailbox::run` の内部で `.await` するのは避けたほうがよい。  
理由は、mailbox drain は suspend / resume / system message priority / throughput / cleanup の整合性境界だからである。

```
Tokio / Embassy task
  |
  v
Executor adapter
  |
  v
MessageDispatcherShared::register_for_execution
  |
  v
Mailbox::run(...)  // non-awaiting drain boundary
  |
  v
MessageInvoker::invoke(...)
```

async はこの図の上下に入れるのがよい。

- 上側: executor / tick driver / timer / wakeup signal
- 下側: `pipe_to_self` / `pipe_to` が future を起動し、完了結果を mailbox message として戻す

中央の mailbox drain に `.await` を入れると、lock discipline、reentrancy、supervision、stash、cleanup が一気に async state machine 化される。  
そこは価値に対して破壊範囲が大きすぎる。

## 6. async 化の選択肢

### 選択肢 A: runtime adapter を async-first にする

#### 内容

- default Tokio executor は `tokio::spawn(async move { task(); })` 相当の task 実行へ寄せる
- blocking 用には別 executor を用意し、`DispatcherSelector::Blocking` から明示的に選ばせる
- Embassy adapter は static task + signal + bounded ready queue で mailbox drain を駆動する
- `Mailbox::run` / `MessageDispatcherShared` / `Actor::receive` は sync contract のまま維持する

#### 利点

- Tokio / Embassy の runtime 資産を活かせる
- full async core より影響範囲が小さい
- `spawn_blocking` 既定の過剰保守を外せる
- embedded 側でも「thread がある前提」に寄らない

#### 欠点

- default dispatcher 上の actor は blocking してはならない、という contract を明文化する必要がある
- sync actor が重い処理をすると Tokio worker / Embassy executor を占有し得る
- executor selector と cookbook の整備が必要になる

#### 評価

**固定前提では第一推奨**。  
最も少ない破壊で「Tokio / Embassy を使うなら async 化しないともったいない」という意図に応えられる。

### 選択肢 B: `pipe_to_self` / `pipe_to` を future-to-message adapter として厚くする

#### 内容

- 既存 `ActorContext::pipe_to_self` / `pipe_to` と `TypedActorContext::pipe_to_self` / `pipe_to` を Pekko `pipeToSelf` 型の async adapter として明文化する
- 実装順序は untyped kernel first とし、`ActorContext` / `ContextPipeTask` / delivery 観測を固定してから typed wrapper を整える
- actor handler は future を返さず、同期的に future を登録して `Behavior` / `Result` を返す
- future completion は typed message として mailbox に戻し、state 更新は completion message handler で行う
- ask / ask_with_status / delivery / timer と future-to-message の戻り方を統一する

#### 利点

- actor 利用者の async ergonomics が大きく上がる
- Tokio / Embassy 前提と自然に整合する
- mailbox drain の整合性境界を保てる
- Pekko typed と同じ「同期 actor 記述 + future completion message」の境界を保てる
- 既存 kernel `pipe_to_self` を壊さず、その上に typed surface の docs / tests / failure observability を厚くできる

#### 欠点

- async I/O 中心の actor では completion message 型を明示的に設計する必要がある
- in-flight future の cancel / restart / stale result discard は generation token など利用者側プロトコルで扱う必要がある
- `pipe_to_self` の mapper / adapter failure を観測可能にする tests と docs が必要になる

#### 評価

**第二推奨**。  
runtime adapter async-first 化の次に取り組む価値が高い。ただしこれは handler が `Future` を返す新 contract を作る話ではなく、既存 `pipe_to_self` / `pipe_to` を Pekko 互換の future-to-message surface として磨く話である。

### 選択肢 C: dispatcher / executor trait を future submit へ変える

#### 内容

- `Executor::execute(Box<dyn FnOnce()>)` を future submit へ置き換える、または併設する
- `MessageDispatcherShared::register_for_execution` が async task を直接 submit する
- mailbox drain 自体は sync closure として future 内で実行する

#### 利点

- Tokio / Embassy の spawn API に型として寄せやすい
- executor adapter の意図は明確になる

#### 欠点

- `mailbox.run` が sync のままなら、本質的には closure を future に包むだけになりやすい
- Embassy では dynamic future spawn が簡単とは限らず、static task + queue のほうが合う可能性がある
- executor trait の破壊的変更に対する利益が選択肢 A より小さい

#### 評価

**選択肢 A の実装を進めてから再評価**。  
trait を急いで future 化するより、まず runtime-specific executor adapter を分けるほうが情報が増える。

### 選択肢 D: actor invocation まで含めて full async core にする

#### 内容

- `Actor::receive` を async 化
- `MessageInvoker::invoke` を async 化
- mailbox が「1 message = 1 future poll / wake」モデルに変わる
- suspend / resume / restart / stash / death watch / mailbox pressure がすべて async state machine と絡む

#### 利点

- actor handler が自然に `await` できる
- I/O 中心の actor にとって記述性が高い

#### 欠点

- 影響範囲が極めて大きい
- mailbox / dispatcher / actor lifecycle / supervision の意味論がほぼ全面的に変わる
- Pekko parity はかなり薄まる
- `.await` を跨ぐ lock 禁止の規律が core 全体に入る

#### 評価

**現時点では非推奨**。  
Tokio / Embassy 前提でも、この変更は「async 化」ではなく「runtime model の刷新」に近い。

## 7. どこまでを妥当な非互換とみなすか

### 7.1 妥当とみなせるもの

- bounded mailbox を overflow strategy 中心で設計する
- `pushTimeOut` 系 blocking bounded mailbox を core の既定にしない
- blocking workload は `Blocking` dispatcher / blocking executor へ明示的に隔離する
- mailbox drain は non-awaiting のまま保ち、async は runtime adapter と actor-facing API で受ける

これは「Pekko を Rust へ翻訳する」のではなく、**Pekko の意味論を保ちつつ Rust 向けに整理する** という意味で妥当である。

特に Embassy 前提では、blocking mailbox 互換を厚くするより、async backpressure と pressure notification を整えるほうが自然である。

### 7.2 妥当性の条件

ただし、これを妥当とするなら次を明文化する必要がある。

1. default dispatcher 上の actor は blocking I/O をしてはならない
2. blocking actor は `DispatcherSelector::Blocking` などで明示的に隔離する
3. bounded mailbox の overflow は block ではなく policy / dead letter / pressure event で観測する
4. async I/O は `pipe_to_self` / `pipe_to` 系で mailbox message へ戻す
5. Embassy adapter は thread blocking を前提にしない

何も書かないままだと、bounded mailbox が部分対応なのか、Tokio / Embassy 前提の意図的非互換なのかが曖昧になる。

## 8. 推奨判断

### 推奨 1: default executor を async runtime task 寄りに分ける

固定前提では、`TokioExecutor = spawn_blocking` を既定とみなす設計は再考する。  
default dispatcher は Tokio task executor へ寄せ、blocking dispatcher は Tokio blocking executor へ分ける。

Embassy 側も同じ考え方で、blocking ではなく static task / signal / bounded ready queue による mailbox drain を設計する。

### 推奨 2: `pipe_to_self` / `pipe_to` surface を次の主戦場にする

`pipe_to_self` は良い基礎であり、Pekko typed でも同じく actor 記述は同期的なまま `pipeToSelf` で Future completion を message 化する。  
そのため、まず untyped kernel の `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker、delivery failure 観測を固定する。  
その上で typed 側は handler が `Future` を返す新 contract を増やすより、既存 `TypedActorContext::pipe_to_self` / `pipe_to` を薄い wrapper として docs、tests、adapter failure observability、cookbook を厚くする価値が高い。

future は actor state を借用し続けず、owned value に閉じ込める。  
完了結果は message として戻し、state 更新は通常の同期 handler で行う。

### 推奨 3: mailbox / dispatcher core の full async 化は保留する

`Actor::receive` / `MessageInvoker::invoke` / `Mailbox::run` の async 化は、supervision と mailbox scheduling の意味論を大きく変える。  
Tokio / Embassy 前提でも、まずは runtime adapter と `pipe_to_self` / `pipe_to` surface を厚くしてから必要性を再評価する。

### 推奨 4: blocking bounded mailbox は低優先度に落とす

Pekko `pushTimeOut` 互換は、固定前提では優先度が下がる。  
必要なら std 限定の compatibility option として後から足せるが、先にやるべきは async task executor、Embassy adapter、future-to-message adapter surface である。

## 9. 次に詰めるべき具体論点

1. `TokioExecutor` の責務をどう分けるか
   - 既定を task executor にする
   - blocking 用を別名 / 別 factory にする
   - `DispatcherSelector::Blocking` の解決先を blocking executor に固定する
2. Tokio current-thread runtime をサポート対象に入れるか
   - lock / reentrancy / blocking 禁止の規律が強いため、初期は multi-thread runtime 前提が安全
3. Embassy adapter の実行モデルをどうするか
   - static worker task
   - signal wakeup
   - bounded ready queue
   - `embassy-time` による scheduler tick
4. `pipe_to_self` / `pipe_to` の cancel / restart / stop semantics をどう文書化するか
   - actor restart 時に in-flight future を cancel するか
   - stop 時に completion message を捨てるか dead letter にするか
   - supervision error と future error をどう対応づけるか
5. backpressure をどの API で表現するか
   - mailbox overflow policy
   - mailbox pressure notification
   - typed delivery / pull protocol
   - ask timeout

## 10. 現時点の意思決定案

現時点で最もバランスが良い案は次である。

- **mailbox drain / actor invocation の core contract は sync / non-awaiting のまま維持**
- **std の default dispatcher は Tokio task executor へ寄せる**
- **blocking は別 executor と `Blocking` dispatcher selector で明示的に隔離**
- **embedded は Embassy adapter を新設し、static task + signal + timer で駆動**
- **async I/O ergonomics は untyped kernel の `pipe_to_self` / `pipe_to` を先に固定し、その上に typed thin wrapper を整える**
- **Pekko `pushTimeOut` 互換は低優先度の optional compatibility とする**

この案なら、

- 既存の Pekko parity の強みを保てる
- Tokio / Embassy の runtime 資産を活かせる
- mailbox hot path に `.await` を持ち込まない
- blocking と async の責務境界が明確になる
- 将来 full async core が本当に必要になった場合も、前段の adapter / future-to-message surface 整備が土台になる

という利点がある。
