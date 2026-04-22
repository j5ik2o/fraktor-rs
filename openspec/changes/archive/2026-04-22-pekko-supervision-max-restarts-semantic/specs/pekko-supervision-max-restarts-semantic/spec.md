## ADDED Requirements

### Requirement: restart 上限は Pekko `maxNrOfRetries` の 3 値契約と同型でなければならない

kernel `SupervisorStrategy` の restart 上限は、以下の Pekko `maxNrOfRetries` (参照: `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala:56-62`) と同型の 3 値契約を表現する型で保持しなければならない (MUST):

- `Unlimited`: 無制限 (Pekko `maxNrOfRetries = -1` ⇒ `(None, _)` arm ⇒ 常に permit)
- `WithinWindow(0)`: retry なし、`requestRestartPermission` を呼んだ瞬間に false (Pekko `(Some(0), _) if retries < 1 => false`)
- `WithinWindow(n)` (n > 0): Pekko `(Some(n), _)` arm。window の有無で分岐 (Requirement 「handle_failure は Pekko directive 契約どおりに accumulator を更新しなければならない」を参照)

kernel 層に `u32` 単体や `i32 + -1 マジック値` / `0 = unlimited` の反転意味表現を残してはならない (MUST NOT)。

#### Scenario: RestartLimit 型は 3 variant を持ち Pekko 契約に対応づけられる

- **WHEN** `modules/actor-core/src/core/kernel/actor/supervision/` 配下に新設される `RestartLimit` 型定義を確認する
- **THEN** `Unlimited` variant と `WithinWindow(u32)` variant の 2 variant を持つ enum として定義されている
- **AND** rustdoc コメントで Pekko `maxNrOfRetries = -1` / `= 0` / `> 0` との対応が明示されている
- **AND** `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` が付与されている

#### Scenario: SupervisorStrategy::max_restarts は RestartLimit を保持する

- **WHEN** `SupervisorStrategy` の型定義を確認する
- **THEN** `max_restarts: RestartLimit` フィールドを持つ
- **AND** `u32` 型の `max_restarts` フィールドは存在しない

#### Scenario: BackoffSupervisorStrategy も RestartLimit を共有する

- **WHEN** `BackoffSupervisorStrategy` / `BackoffSupervisorStrategyConfig` / 関連 builder の型定義を確認する
- **THEN** restart 上限は `RestartLimit` 型で保持される
- **AND** `u32` 単体で retry 上限を表現する経路は残っていない

### Requirement: handle_failure は Pekko directive 契約どおりに accumulator を更新しなければならない

`SupervisorStrategy::handle_failure` は decider が返した `SupervisorDirective` に応じて以下の契約で `RestartStatistics` を更新しなければならない (MUST)。contract はすべて `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala:56-86` (`ChildRestartStats.requestRestartPermission` / `retriesInWindowOkay`) と行単位で一致する。

- `Restart`: `RestartStatistics::request_restart_permission(now, self.max_restarts, self.within)` を呼ぶ。戻り値が `true` なら `Restart`、`false` なら `statistics.reset()` の後 `Stop` に昇格
- `Stop`: `statistics.reset()` した後 `Stop`
- `Escalate`: `statistics.reset()` した後 `Escalate`
- `Resume`: **`statistics` には一切触れない** (Pekko `FaultHandling` の `Resume` 分岐は `childStats` を更新しない)

#### Scenario: Unlimited + window == ZERO は counter を更新せず常に Restart を返す

- **GIVEN** `max_restarts: RestartLimit::Unlimited`, `within: Duration::ZERO` の `SupervisorStrategy`
- **AND** `decider` が常に `Restart` を返す
- **WHEN** `handle_failure` を任意の timeline (例: `now = 0s, 5s, 1000s, ...`) で連続 100 回呼び出す
- **THEN** 100 回とも `SupervisorDirective::Restart` が返る
- **AND** 呼び出し後の `RestartStatistics::restart_count()` は `0` のまま (Pekko `(None, _) => true` arm は counter 非更新)
- **AND** `RestartStatistics::reset()` は本 handler 内で呼ばれていない

#### Scenario: Unlimited + window > 0 は Pekko `retriesInWindowOkay(1, window)` 経路で window 内 2 回目以降は Stop へ昇格する (Pekko 仕様の quirk を直訳)

- **GIVEN** `max_restarts: RestartLimit::Unlimited`, `within: Duration::from_secs(10)` の `SupervisorStrategy`
- **AND** `decider` が `Restart` を返す
- **NOTE** Pekko `FaultHandling.scala:60` (`case (x, Some(window)) => retriesInWindowOkay(if (x.isDefined) x.get else 1, window)`) は `Unlimited + window > 0` で `retries = 1` を渡す。そのため window 内 2 回目で `retriesDone = 2 > retries = 1` となり false を返す。Pekko デフォルトの `OneForOneStrategy` は `(maxNrOfRetries = -1, withinTimeRange = Duration.Inf)` で `(None, None)` arm ⇒ 常に true となるため、この quirk は `Unlimited + finite window` を明示指定した場合にのみ顕在化する。本 change は Pekko parity のためこの挙動を直訳する
- **WHEN** `now = 0s` で `handle_failure` を呼ぶ
- **THEN** `SupervisorDirective::Restart` が返る
- **AND** `restart_count() == 1`、`window_start == Some(0s)` (Pekko: 初回 windowStart を設定、`retriesDone = 1 <= 1 = true`)
- **WHEN** `now = 5s` (window 内 2 回目) で `handle_failure` を呼ぶ
- **THEN** `SupervisorDirective::Stop` が返る (Pekko: `retriesDone = 2 > 1 = false` ⇒ `request_restart_permission = false` ⇒ `handle_failure` が `statistics.reset()` + Stop)
- **AND** `restart_count() == 0`、`window_start == None` (reset 済み)
- **WHEN** `now = 20s` (直前の reset で `window_start == None` となった初回起動状態から) で `handle_failure` を呼ぶ
- **THEN** `SupervisorDirective::Restart` が返る (Pekko `retriesInWindowOkay` 冒頭の `windowStart == 0` 初期化分岐 `FaultHandling.scala:73-76`: `window_start = now` を設定して `retriesDone = 1 <= 1 = true`)
- **AND** `restart_count() == 1`、`window_start == Some(20s)`

#### Scenario: Unlimited + window > 0 で window expire を挟むと再び permit される

- **GIVEN** `max_restarts: RestartLimit::Unlimited`, `within: Duration::from_secs(10)` の `SupervisorStrategy`
- **AND** `RestartStatistics` が **直接** `restart_count = 1` / `window_start = Some(Duration::ZERO)` に初期化された状態 (Scenario 間の順序依存を避けるため独立の事前状態として明示)
- **AND** `decider` が `Restart` を返す
- **WHEN** `now = 15s` (`window_start = 0s` + `within = 10s` より `15s > 10s` で window 外) で `handle_failure` を呼ぶ
- **THEN** `SupervisorDirective::Restart` が返る (Pekko `retriesInWindowOkay` の window expire 分岐 `FaultHandling.scala:81-85`: `count = 1` + `window_start = now` + `return true`)
- **AND** `restart_count() == 1`、`window_start == Some(15s)`

#### Scenario: WithinWindow(0) は request_restart_permission で false を返し counter 更新なしで Stop する

- **GIVEN** `max_restarts: RestartLimit::WithinWindow(0)`, `within: Duration::from_secs(10)` の `SupervisorStrategy`
- **AND** `decider` が `Restart` を返す
- **AND** `RestartStatistics` が `restart_count == 0` / `window_start == None` (初期状態)
- **WHEN** `handle_failure` を 1 回呼び出す
- **THEN** `request_restart_permission` は `(WithinWindow(0), _)` arm で **counter / window_start を一切更新せず** `false` を返す (Pekko `(Some(0), _) if retries < 1 => false`)
- **AND** `handle_failure` は `SupervisorDirective::Stop` を返す
- **AND** `handle_failure` が `statistics.reset()` を呼ぶ (Pekko 本体は `reset` を呼ばず代わりに `processFailure(false, ...)` → `context.stop(child)` で child を停止し stats を死亡させる。fraktor-rs は child 停止を直接表現せず、`reset()` によって state を「child 死亡後の初期状態」と等価に保つ意図的差異。最終 state としては Pekko と同値)
- **AND** 呼び出し後の `restart_count() == 0`、`window_start() == None`

#### Scenario: WithinWindow(n) + window == ZERO はカウンタを増やし n を超えると Stop する

- **GIVEN** `max_restarts: RestartLimit::WithinWindow(3)`, `within: Duration::ZERO`
- **AND** `decider` が `Restart` を返す
- **WHEN** `handle_failure` を 4 回呼ぶ (`now` は任意)
- **THEN** 1 回目: `Restart`、`restart_count == 1`
- **AND** 2 回目: `Restart`、`restart_count == 2`
- **AND** 3 回目: `Restart`、`restart_count == 3` (Pekko `(Some(3), None) => count += 1; count <= 3`)
- **AND** 4 回目: `Stop` (`restart_count` は一時的に `4` に増えた後、`request_restart_permission` が `false` を返し `handle_failure` が `statistics.reset()` を呼ぶため最終的に `0`)

#### Scenario: WithinWindow(n) + window > 0 は Pekko one-shot window で window expire 時にリセット + permit を返す

- **GIVEN** `max_restarts: RestartLimit::WithinWindow(3)`, `within: Duration::from_secs(10)` の `SupervisorStrategy`
- **AND** `decider` が `Restart` を返す
- **WHEN** `now = 0s` で `handle_failure` を呼ぶ
- **THEN** `Restart`、`restart_count == 1`、`window_start == Some(0s)`
- **WHEN** `now = 3s, 6s, 9s` でそれぞれ `handle_failure` を呼ぶ (window 内 3 回)
- **THEN** `now = 3s`: `Restart`、`restart_count == 2`
- **AND** `now = 6s`: `Restart`、`restart_count == 3`
- **AND** `now = 9s`: `Stop` (Pekko `retriesDone=4 <= 3 = false` ⇒ `request_restart_permission = false`)。`handle_failure` が `statistics.reset()` を呼び `restart_count == 0`、`window_start == None`
- **WHEN** `now = 12s` で次の `handle_failure` を呼ぶ
- **THEN** `Restart` (reset 後の新しい window 開始)、`restart_count == 1`、`window_start == Some(12s)`

#### Scenario: window expire (window 外) 時は Pekko と同じく counter=1 + window_start=now + permit

- **GIVEN** `max_restarts: RestartLimit::WithinWindow(3)`, `within: Duration::from_secs(10)`
- **AND** `restart_count == 2`、`window_start == Some(0s)` の状態 (事前に `now = 0s, 5s` で 2 回 permit 済み)
- **WHEN** `now = 15s` で `handle_failure` を呼ぶ (`15s > 0s + 10s` で window 外)
- **THEN** `Restart` が返る
- **AND** `restart_count == 1` (`retriesDone = 3` ではなく `1` にリセット、Pekko `FaultHandling.scala:82`)
- **AND** `window_start == Some(15s)` (Pekko `FaultHandling.scala:83`)

#### Scenario: decider が Stop を返すと統計はリセットされる

- **GIVEN** `handle_failure` 呼び出しで `restart_count > 0` / `window_start == Some(_)` の状態
- **WHEN** `decider` が `Stop` を返す
- **THEN** `SupervisorDirective::Stop` が返る
- **AND** `restart_count() == 0`、`window_start() == None` (reset 済み)

#### Scenario: decider が Escalate を返すと統計はリセットされる

- **GIVEN** `handle_failure` 呼び出しで `restart_count > 0` / `window_start == Some(_)` の状態
- **WHEN** `decider` が `Escalate` を返す
- **THEN** `SupervisorDirective::Escalate` が返る
- **AND** `restart_count() == 0`、`window_start() == None`

#### Scenario: decider が Resume を返すと統計は完全に維持される

- **GIVEN** `handle_failure` 呼び出しで `restart_count == 5` / `window_start == Some(10s)` の状態
- **WHEN** `decider` が `Resume` を返す
- **THEN** `SupervisorDirective::Resume` が返る
- **AND** `restart_count() == 5`、`window_start() == Some(10s)` (Pekko: `Resume` は childStats に触れない)
- **AND** その後 `decider` が `Restart` を返す呼び出しがあれば、上記 state から継続して Pekko `requestRestartPermission` が評価される

### Requirement: RestartStatistics は Pekko one-shot window の内部 state に書き直されなければならない

`RestartStatistics` の内部 state は `Vec<Duration>` の sliding window 履歴から、Pekko `ChildRestartStats` (`FaultHandling.scala:48-86`) と同型の **`restart_count: u32` + `window_start: Option<Duration>`** のペアに置換されなければならない (MUST)。sliding window 用の `prune` / `failures_within` / `failures` フィールドは削除する (MUST)。

公開 API として `request_restart_permission(now: Duration, limit: RestartLimit, window: Duration) -> bool` を提供し、Pekko `requestRestartPermission` と行単位で一致する分岐ロジックを実装する (MUST)。`record_failure(now, window, max_history) -> usize` / `failures_within` は削除する (MUST)。

`now: Duration` 引数は **monotonic clock** (`ActorSystem::monotonic_now()` 由来、システム起動後経過時間) でなければならない (MUST)。Pekko は `System.nanoTime()` (monotonic) を使用 (`FaultHandling.scala:71`) しており、wall clock (UTC 絡み) を渡すと system clock の巻き戻りで window 判定が破綻するため禁止。fraktor-rs の呼び出し元 `ActorCell::handle_child_failure` は既に `self.system().monotonic_now()` (`actor_cell.rs:1365`) を渡しており、本 Requirement はこの既存契約を spec 化する。

#### Scenario: RestartStatistics の内部表現は count + window_start になる

- **WHEN** `RestartStatistics` の構造体定義を確認する
- **THEN** `restart_count: u32` フィールドを持つ
- **AND** `window_start: Option<Duration>` フィールドを持つ
- **AND** `failures: Vec<Duration>` フィールドは存在しない
- **AND** `prune` / `failures_within` メソッドは存在しない

#### Scenario: request_restart_permission は Pekko 4 分岐を再現する

- **WHEN** `RestartStatistics::request_restart_permission` のシグネチャと実装を確認する
- **THEN** シグネチャは `(&mut self, now: Duration, limit: RestartLimit, window: Duration) -> bool`
- **AND** 実装は以下 5 分岐を持つ (Pekko `FaultHandling.scala:56-62` の 4 case arm + `Duration.ZERO` 分岐を `window.is_zero()` で明示化):
  - `(WithinWindow(0), _) => false` (counter / window_start 非更新、Pekko `(Some(0), _) if retries < 1 => false`)
  - `(Unlimited, window.is_zero()) => true` (counter / window_start 非更新、Pekko `(None, _) => true`)
  - `(WithinWindow(n), window.is_zero()) => count.saturating_add(1); count <= n` (Pekko `(Some(n), None) => count += 1; count <= n`)
  - `(Unlimited, window > 0) => retries_in_window_okay(1, window, now)` (Pekko `(None, Some(w)) => retriesInWindowOkay(1, w)`、`retries = 1` 固定)
  - `(WithinWindow(n), window > 0) => retries_in_window_okay(n, window, now)` (Pekko `(Some(n), Some(w)) => retriesInWindowOkay(n, w)`)

#### Scenario: retries_in_window_okay は Pekko one-shot window を直訳する

- **WHEN** `retries_in_window_okay` の実装を確認する
- **THEN** `retries_done = restart_count.saturating_add(1)` で候補値を作成
- **AND** `window_start` が `None` なら `Some(now)` に設定
- **AND** `now - window_start <= window` (window 内) なら `restart_count = retries_done` を書き戻し `retries_done <= retries` を返す
- **AND** window 外なら `restart_count = 1` + `window_start = Some(now)` + `true` を返す (Pekko: carryover せず新 window 開始、permit)

#### Scenario: reset は count と window_start を同時にクリアする

- **WHEN** `RestartStatistics::reset()` を呼ぶ
- **THEN** `restart_count() == 0`
- **AND** `window_start() == None`

### Requirement: typed 層 API は Pekko 直訳の 2 メソッドで restart 上限を指定しなければならない

`RestartSupervisorStrategy` および `BackoffSupervisorStrategy` の typed 層 DSL は、restart 上限を指定する API として以下 2 メソッドを提供しなければならない (MUST):

- `with_limit(max_restarts: u32, within: Duration) -> Self`: Pekko `maxNrOfRetries = max_restarts` (`0` を含む有限値)
- `with_unlimited_restarts(within: Duration) -> Self`: Pekko `maxNrOfRetries = -1`

`i32 + -1 マジック値 + 0 でのパニック` を使う古い API (`with_limit(i32, Duration)`) は廃止されなければならない (MUST)。`within` 引数の `Duration::ZERO` は Pekko `Duration.Inf` に相当する「window なし」センチネルであり、rustdoc で明示しなければならない (MUST)。

#### Scenario: with_limit(0, within) は panic せず「retry なし」として受理される

- **WHEN** `RestartSupervisorStrategy::default().with_limit(0, Duration::from_secs(10))` を呼ぶ
- **THEN** panic せず `RestartSupervisorStrategy` を返す
- **AND** kernel 側の `SupervisorStrategy::max_restarts` が `RestartLimit::WithinWindow(0)` になっている

#### Scenario: with_limit(n, within) は RestartLimit::WithinWindow(n) を構築する

- **WHEN** `RestartSupervisorStrategy::default().with_limit(3, Duration::from_secs(10))` を呼ぶ
- **THEN** kernel 側の `SupervisorStrategy::max_restarts` が `RestartLimit::WithinWindow(3)` になっている

#### Scenario: with_unlimited_restarts(within) は RestartLimit::Unlimited を構築する

- **WHEN** `RestartSupervisorStrategy::default().with_unlimited_restarts(Duration::from_secs(10))` を呼ぶ
- **THEN** kernel 側の `SupervisorStrategy::max_restarts` が `RestartLimit::Unlimited` になっている

#### Scenario: 古い i32 シグネチャは存在しない

- **WHEN** `modules/actor-core/src/core/typed/restart_supervisor_strategy.rs` と `backoff_supervisor_strategy.rs` を grep する
- **THEN** `fn with_limit(self, max_restarts: i32, ...)` のシグネチャは存在しない
- **AND** `max_restarts must be -1 or at least 1` という panic メッセージを含む `assert!` / `panic!` は存在しない
- **AND** `max_restarts == -1` のような `i32` magic value 比較は存在しない

#### Scenario: Duration::ZERO と Pekko Duration.Inf の対応が rustdoc に明記される

- **WHEN** `with_limit` / `with_unlimited_restarts` / `SupervisorStrategy::new` / `RestartLimit` / `RestartStatistics::request_restart_permission` の rustdoc を確認する
- **THEN** `within: Duration::ZERO` が Pekko `Duration.Inf` (= window なし、無期限累計) に対応することを明示する文言が含まれる
- **AND** Pekko `Duration.Zero` (= 0ms window) との意味乖離を注意書きとして含む

### Requirement: gap-analysis の SP-M1 項目は本 change で done 化されなければならない

`docs/gap-analysis/actor-gap-analysis.md` の SP-M1 行は本 change のマージに伴い `~~medium~~ done` にマークされ、残存 medium 件数が更新されなければならない (MUST)。

#### Scenario: SP-M1 行が done 状態に更新される

- **WHEN** `docs/gap-analysis/actor-gap-analysis.md` の SP-M1 行を確認する
- **THEN** 深刻度欄に `~~medium~~ done` または同等のマーカーが含まれる
- **AND** 本 change の archive 名 (`2026-04-22-pekko-supervision-max-restarts-semantic` もしくは実際のアーカイブ日付) への参照が追記されている

#### Scenario: 残存 medium 件数が減算される

- **WHEN** gap-analysis `まとめ` セクションの残存内部セマンティクス数値を確認する
- **THEN** `medium 13` から `medium 12` へ減算されている
