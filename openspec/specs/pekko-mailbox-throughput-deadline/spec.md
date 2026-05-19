# pekko-mailbox-throughput-deadline Specification

## Purpose
TBD - created by archiving change pekko-mailbox-throughput-deadline. Update Purpose after archive.
## Requirements
### Requirement: Mailbox::run() は throughput 本数と throughput deadline の合成条件で yield しなければならない

kernel `Mailbox::run(throughput, throughput_deadline)` は Pekko `Mailbox.scala:261-278` が規定する合成条件 `left > 0 && (throughput_deadline is None || monotonic_now() < deadline_at)` を `process_mailbox` のループ条件として enforce しなければならない (MUST)。どちらか一方が崩れた時点でループを即時 break し、`finish_run()` の reschedule 判定へ進まなければならない。`throughput_deadline = None` の場合は throughput 本数のみで yield する従来挙動を完全に保たなければならない (MUST)。

#### Scenario: throughput 未消化でも deadline 超過で yield する

- **GIVEN** `Mailbox::run(throughput = 100, throughput_deadline = Some(Duration::from_millis(10)))`
- **AND** 各メッセージ処理に 5ms 以上掛かるアクター
- **WHEN** mailbox に 100 通の user メッセージが積まれた状態で `run()` を呼ぶ
- **THEN** 10ms を超えた時点でループから抜ける
- **AND** `run()` の戻り値 (reschedule 要求) が `true` となり dispatcher は後続 drain を再スケジュールする
- **AND** 未処理の user メッセージは queue に残ったままとなる

#### Scenario: deadline が None のときは throughput を消化しきるまで続行する

- **GIVEN** `Mailbox::run(throughput = 100, throughput_deadline = None)`
- **AND** 各メッセージ処理に 5ms 以上掛かるアクター
- **WHEN** mailbox に 100 通の user メッセージが積まれた状態で `run()` を呼ぶ
- **THEN** 100 通全て処理してから yield する (throughput-only 挙動、従来通り)
- **AND** 経過時間に関わらず途中で yield しない

#### Scenario: deadline 未達で throughput 消化の場合は throughput 基準で yield する

- **GIVEN** `Mailbox::run(throughput = 10, throughput_deadline = Some(Duration::from_secs(60)))`
- **AND** 各メッセージ処理が 1µs 未満の軽量アクター
- **WHEN** mailbox に 20 通の user メッセージが積まれた状態で `run()` を呼ぶ
- **THEN** 10 通処理した時点で yield する
- **AND** deadline は未達だが throughput 上限で抜ける

#### Scenario: throughput = 1 のときは deadline の有無が挙動を変えない (Pekko `left > 1` の意図保持)

Pekko `Mailbox.scala:275` の `(left > 1) && (...)` は、throughput = 1 のケースで deadline 判定を
評価せず 1 メッセージ処理後に再帰しない挙動を規定する。fraktor-rs 実装は while ループで
`left -= 1` 後に break 判定を行うため、`left = 1 → left = 0` で while 条件が先に false になり
同じ結果に到達する。

- **GIVEN** `Mailbox::run(throughput = 1, throughput_deadline = Some(Duration::ZERO))`
- **AND** deadline 計算上、`self.clock.as_ref()` の呼び出し結果 + `Duration::ZERO` は直ちに deadline 到達
- **AND** mailbox に 10 通の user メッセージが積まれている
- **WHEN** `run()` を呼ぶ
- **THEN** 1 通目が処理されてから yield する (deadline ZERO による途中 break ではなく throughput=1 消化で抜ける)
- **AND** 未処理 9 通は queue に残り、`run()` 戻り値は reschedule 要求 `true`

#### Scenario: deadline = Some(Duration::ZERO) は 1 件処理後に break する (clock 進行あり)

`Mailbox::run()` 先頭で `deadline_at = Some(clock_start + Duration::ZERO)` (= `Some(clock_start)`)
を計算するため、ループ初回は `clock_now < deadline_at` が成立しうる (clock 進行前) が、
1 通処理後の iteration では clock が進んで deadline 到達となる。Pekko `left > 1 && (nanoTime - deadlineNs) < 0`
も同じく「1 通は必ず処理される、2 通目から deadline 判定で break」という挙動になる。

- **GIVEN** `Mailbox::run(throughput = 10, throughput_deadline = Some(Duration::ZERO))`
- **AND** mailbox に 10 通の user メッセージが積まれている
- **WHEN** `run()` を呼び、各 invoke で clock が 1µs でも進む
- **THEN** 1 通処理後に deadline break に到達する
- **AND** 合計処理数は 1 通、reschedule 要求 `true` で yield する

#### Scenario: deadline = Some(Duration::ZERO) と clock 固定 — deadline 判定の境界動作

mock clock が時間進行しない (fix) 場合、`clock_now < deadline_at` は `deadline_at == clock_now`
の等式となり `>=` 条件で break 判定が true になる。これにより `throughput ≥ 2`
の場合には **1 通処理後 (clock 固定下) でも deadline break に到達する** ことが保証される。
`throughput = 1` の場合は `left = 0` で while 先頭条件が先に false になるため、deadline break
経路は踏まれないが観測結果は等価 (1 通処理済で yield)。

- **GIVEN** `Mailbox::run(throughput = 2, throughput_deadline = Some(Duration::ZERO))`
- **AND** mock clock が進行しない (`advance` を呼ばない)
- **AND** mailbox に 5 通の user メッセージが積まれている
- **WHEN** `run()` を呼ぶ
- **THEN** 1 通処理後に `clock_now >= deadline_at` が成立して break
- **AND** 合計処理数は 1 通 (throughput 限界の 2 通目には到達しない)
- **AND** reschedule 要求 `true` で yield する

### Requirement: deadline 計算に使う clock は monotonic でなければならない

`Mailbox::run()` が内部で取得する現在時刻は Pekko `System.nanoTime()` (`Mailbox.scala:265,275`) 同様に monotonic でなければならない (MUST)。wall-clock 調整による deadline 破綻を避けるため、`ActorSystem::monotonic_now()` 相当の経路で時刻を注入する。内部実装は `std::time::Instant` (std アダプタ) / `embassy-time::Instant` 等の monotonic 時刻源に置き換え可能な形 (trait / callback 注入) で提供し、no_std core を汚染してはならない (MUST)。

#### Scenario: wall-clock が巻き戻っても deadline 判定は不変

- **GIVEN** `Mailbox::run(throughput = 100, throughput_deadline = Some(Duration::from_millis(10)))`
- **AND** 処理の途中で OS の wall-clock が過去に巻き戻される
- **WHEN** mailbox が処理を継続する
- **THEN** monotonic clock は前進し続けるため 10ms 経過で正しく yield する
- **AND** wall-clock 巻き戻しによる deadline 破綻は発生しない

#### Scenario: no_std 環境でも deadline enforcement が動く

- **GIVEN** std feature を無効化した no_std ビルド
- **AND** embedded な monotonic 時刻源を `ActorSystem` に注入
- **WHEN** `Mailbox::run()` が deadline 判定を行う
- **THEN** std::time 依存なしで deadline 比較が成立する
- **AND** `cfg-std-forbid` dylint が違反を検出しない

### Requirement: deadline はループ開始時に一度だけ計算しなければならない

Pekko `Mailbox.scala:262-266` が `deadlineNs = System.nanoTime + throughputDeadlineTime.toNanos` を `processMailbox` のループ先頭で一度だけ評価しているのに準拠し、fraktor-rs 実装も `run()` 呼び出しごとに `deadline_at = now + throughput_deadline` を一度だけ計算しなければならない (MUST)。ループ各イテレーションで再計算してはならない (MUST NOT)。再計算すると deadline が実質無効化され、Pekko の throughput 公平性と乖離する。`Mailbox::clock: Option<MailboxClock>` が `None` または `throughput_deadline` が `None` の場合は `deadline_at = None` となり、deadline 判定自体をスキップしなければならない (MUST、Pekko `isThroughputDeadlineTimeDefined = false` と同値、throughput-only fallback)。

#### Scenario: deadline は `run()` 呼び出し中ずっと一定

- **GIVEN** `Mailbox::run(throughput = 50, throughput_deadline = Some(Duration::from_millis(10)))`
- **WHEN** ループ内で 5 通処理して 8ms 経過、更に 2 通処理して 11ms 経過
- **THEN** 11ms 経過時に deadline (= 開始 + 10ms) を超えていると判定されて yield する
- **AND** `deadline_at` は 5 通目処理後の時点でも同じ値 (ループ開始 + 10ms)

### Requirement: _throughput_deadline プレフィックスは完全に削除しなければならない

`_throughput_deadline: Option<Duration>` のアンダースコアプレフィックス (`base.rs:256`) は「未使用引数」を示すマーカーであり、本 change の完了後は Pekko 同様に使用される引数であるため、`throughput_deadline` (プレフィックスなし) に変更しなければならない (MUST)。`// Deadline support is added in a follow-up change` コメント (`base.rs:294`) も削除し、Pekko `Mailbox.scala:261-278` への参照を含む rustdoc に差し替えなければならない (MUST)。

#### Scenario: 実装完了後は `_throughput_deadline` という識別子が kernel 層に存在しない

- **WHEN** `modules/actor-core/src/core/kernel/` 配下を `_throughput_deadline` で grep
- **THEN** マッチが 0 件
- **AND** `throughput_deadline` (プレフィックスなし) は `run()` の引数として使用されている

#### Scenario: 「follow-up change」コメントが削除されている

- **WHEN** `base.rs` を `Deadline support is added in a follow-up change` で grep
- **THEN** マッチが 0 件
- **AND** 代わりに `Pekko Mailbox.scala:261-278` への参照を含む rustdoc が存在する

