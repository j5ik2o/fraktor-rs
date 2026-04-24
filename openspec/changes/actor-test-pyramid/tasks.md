# Tasks: actor-test-pyramid

## Phase 1: 参照確認とテスト目録

- [x] 1.1 `references/pekko` submodule が読める状態で、以下を確認する:
  - `references/pekko/actor/src/main/scala/org/apache/pekko/`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`
  - `references/pekko/actor-tests/src/test/scala/org/apache/pekko/actor/`
  - `references/pekko/actor-typed-tests/src/test/scala/org/apache/pekko/actor/typed/`
  - `references/pekko/actor-testkit-typed/src/test/scala/org/apache/pekko/actor/testkit/typed/`
- [x] 1.2 classic / typed の主要 spec から、Rust に翻訳すべき観測可能 contract を抽出する。最初に見る代表:
  - classic: `ActorLifeCycleSpec.scala`, `ActorMailboxSpec.scala`, `DeathWatchSpec.scala`, `ReceiveTimeoutSpec.scala`, `FSMActorSpec.scala`, `FSMTimingSpec.scala`, `FSMTransitionSpec.scala`, `SchedulerSpec.scala`, `TimerSpec.scala`
  - typed: `BehaviorSpec.scala`, `ActorContextSpec.scala`, `WatchSpec.scala`, `SupervisionSpec.scala`, `TimerSpec.scala`, `AskSpec.scala`, `EventStreamSpec.scala`
  - typed testkit: `ActorTestKitSpec.scala`, `BehaviorTestKitSpec.scala`, `TestProbeSpec.scala`
- [x] 1.3 submodule が読めない環境では、`docs/gap-analysis/actor-gap-analysis-evidence.md` と既存 `openspec/changes/**` / `openspec/specs/**` の Pekko 参照行を根拠にして作業し、未確認の Pekko 実ファイルを pending として記録する
  - 今回は submodule が読めるため fallback は不要。fallback 方針だけ `docs/plan/actor-test-pyramid.md` に残した。
- [x] 1.4 現行テストを Unit / Contract / Integration / E2E の 4 層に分類する:
  - Unit: `modules/actor-core/src/**/tests.rs` / `foo/tests.rs`
  - Contract: public API / Pekko contract を直接検証する crate 内外テスト
  - Integration: `modules/actor-core/tests/*.rs` / `modules/actor-adaptor-std/tests/*.rs`
  - E2E: public API だけを使う user-flow scenario
- [x] 1.5 Conformance / Regression は独立層ではなく、gap-analysis ID または過去 change に紐づく横断タグとして扱う
- [x] 1.6 `#[ignore]` 付き actor-core テストを棚卸しし、以下に分類する:
  - まだ未実装仕様なので pending のまま残す
  - 実装済みだが ignore が残っているため有効化する
  - scope が古くなっているためテストを削除または書き換える
- [x] 1.7 `scripts/coverage.sh` を実行し、actor 系 coverage から低 coverage ファイルを抽出する。抽出結果は「低 coverage 順」ではなく「Pekko contract 重要度順」に並べ替える

## Phase 2: 配置ルールと fixture 方針

- [x] 2.1 `modules/actor-core/tests/fixtures/` の既存 fixture が compile fixture 置き場であることを確認し、runtime probe helper と混在させない方針を確定する
- [x] 2.2 テストピラミッド方針を `docs/plan/actor-test-pyramid.md` に残す:
  - Unit / Contract / Integration / E2E の分類ルール
  - Conformance / Regression を横断タグとして扱うルール
  - Pekko 代表 Spec と fraktor-rs テスト配置の対応表
  - Wave 1 で扱う contract / integration と follow-up に分ける contract / E2E
  - coverage baseline / Wave 1 目標 / 長期目標
  - fixture / support module の配置ルール
- [x] 2.3 必要な helper は `tests/support/mod.rs` から明示的に module wiring する。候補:
  - `modules/actor-core/tests/support/classic_probe.rs`: sender / receiver / dead letter 観測
  - `modules/actor-core/tests/support/typed_probe.rs`: typed message / signal 観測
  - `modules/actor-core/tests/support/manual_time.rs`: manual tick / scheduler driver 補助
  - `modules/actor-adaptor-std/tests/support/std_system.rs`: std adaptor / tokio executor / logging 補助
  - Wave 1 では新規 helper は不要。必要になった場合の配置ルールだけ固定した。
- [x] 2.4 helper は integration / E2E test crate 内限定に閉じ、production API へ公開しない
- [x] 2.5 helper 名に `Manager` / `Util` / `Service` / `Runtime` / `Engine` などの曖昧サフィックスを使わない
- [x] 2.6 std 依存 helper を `actor-core` production code に入れない。tokio / thread / real time を使う helper は `actor-adaptor-std` または integration / E2E test 側に置く

## Phase 3: Unit 層の Wave 1 実装

- [x] 3.1 Unit 層の対象を、system 起動を不要にできる型単位 / 純粋変換 / 局所状態機械に限定する
- [x] 3.2 coverage と Pekko contract 重要度から、Wave 1 の Unit 追加対象を以下に絞る:
  - actor path parser / path parts / child path の境界値
  - actor ref sender / dead letter / provider の局所契約
  - actor context / actor cell の public accessor と error path
  - std adaptor の leaf helper (`StdClock`, `StdBlocker`, tick driver, tracing subscriber)
- [x] 3.3 Unit 層では `ActorSystem` 起動、tokio runtime、実時間 sleep を増やさない
- [x] 3.4 Unit 層の追加テストは既存の `foo.rs` + `foo/tests.rs` / `tests.rs` 並置パターンへ置く
- [x] 3.5 Unit 層で見つけた E2E でしか検証できない user flow は、Unit に押し込まず E2E follow-up へ送る

## Phase 4: Contract 層の Wave 1 実装

- [x] 4.1 Phase 1 の目録と coverage 結果から、Wave 1 の contract を最大 5 件に確定する
- [x] 4.2 Wave 1 の初期候補は以下から選ぶ。すでに十分な regression があるものは候補から外す:
  - typed `Behaviors.same` / `unhandled` / `stopped`
  - typed `with_timers`
  - ask / pipeToSelf
  - EventStream subchannel / dead letter marker
  - FS-M1 / FS-M2 の external contract
  - public API compile contract の不足分
- [x] 4.3 Wave 1 で選ばなかった contract は `docs/plan/actor-test-pyramid.md` の follow-up 表に残し、この change では実装しない
- [x] 4.4 Wave 1 の各 contract test は、Pekko reference / gap ID / fraktor-rs module のいずれかをテスト名またはコメントから辿れるようにする
- [x] 4.5 Wave 1 の追加後、同じカテゴリの重複テストが増えすぎていないことを確認する

## Phase 5: Integration 層の Wave 1 実装

- [x] 5.1 Integration 層の対象を、複数 module の接続漏れを検出する scenario に限定する
- [x] 5.2 Integration scenario は最大 2 件に限定する
- [x] 5.3 初期候補は以下から選ぶ。Wave 1 contract と重複する場合は integration 側を削る:
  - classic: spawn → tell → watch → stop → dead letter 観測
  - typed: spawn → message adapter → ask / pipeToSelf → stop
  - std adaptor: tokio executor → dispatcher 起動 → logging subscriber
- [x] 5.4 Wave 1 では std adaptor と core の接続確認を優先し、dispatcher factory / mailbox clock の実配線を検証する
- [x] 5.5 実時間 sleep を避け、manual tick / start_paused / deterministic probe で検証する。避けられない場合は理由をコメントに残す
- [x] 5.6 Integration 層は接続漏れを検出する範囲に絞り、Unit / Contract で済むケースを重複して増やさない

## Phase 6: E2E 層の Wave 1 棚卸し

- [x] 6.1 E2E 層の対象を、public API だけでユーザー操作に近い流れを検証する scenario に限定する
- [x] 6.2 既存 E2E 相当テストを棚卸しする:
  - `actor_path_e2e`
  - `ping_pong`
  - `supervisor`
  - `system_lifecycle`
  - `system_events`
  - `sp_h1_5_system_escalation`
- [x] 6.3 Wave 1 では新規 E2E scenario を追加しない。coverage 目標達成のために E2E を厚くせず、Unit / Contract / Integration の不足だけを埋める
- [x] 6.4 次 wave の E2E 受け入れ条件を `docs/plan/actor-test-pyramid.md` の follow-up 表に残す:
  - classic E2E user flow
  - typed E2E user flow
  - std adaptor E2E boot flow
- [x] 6.5 E2E で検証すべき user flow を Unit / Contract / Integration に分散して実装済み扱いにしない

## Phase 7: Conformance / Regression 横断タグの整備

- [x] 7.1 gap-analysis done 項目のうち、テスト名またはコメントから ID を辿れないものを洗い出す
- [x] 7.2 Conformance / Regression は独立層ではなく横断タグとして扱い、Unit / Contract / Integration / E2E のいずれかへ配置する
- [x] 7.3 本 change で紐づける regression は Wave 1 に選んだ contract に限定する
- [x] 7.4 Wave 1 以外の done 項目は `docs/plan/actor-test-pyramid.md` の follow-up 表に残し、この change では実装しない
- [x] 7.5 既存テストに ID コメントを追加する場合、テストの意図が既に明確な箇所だけに限定する。雑なコメント増量はしない
- [x] 7.6 AC-M4b は remote / cluster 側に依存するため、本 change では pending として記録し、actor-core 単体で無理に再現しない
- [x] 7.7 既存 ignored test のうち実装済み契約に変わったものは Wave 1 に関係するものだけ有効化し、失敗する場合は根本原因を調査して修正する

## Phase 8: Coverage とテスト実行時間の確認

- [x] 8.1 `rtk cargo test -p fraktor-actor-core-rs --lib` を実行し、Unit / Contract 層の失敗がないことを確認する
- [x] 8.2 `rtk cargo test -p fraktor-actor-core-rs --tests` を実行し、Integration / E2E / external contract 層の失敗がないことを確認する
- [x] 8.3 `rtk cargo test -p fraktor-actor-adaptor-std-rs --features test-support` を実行し、std adaptor 側の Unit / Integration / E2E 相当テストの失敗がないことを確認する
- [x] 8.4 `rtk scripts/coverage.sh` を実行し、actor 系 HTML レポートを生成する
- [x] 8.5 Wave 1 coverage を計測し、目標との差分を確認する
- [x] 8.6 Wave 1 coverage 目標を達成する:
  - Function coverage: 85% 以上
  - Line coverage: 85% 以上
  - Region coverage: 84% 以上
  - 実測: Function 86.74% / Line 85.35% / Region 84.72%。Wave 1 目標を達成済み。
- [x] 8.7 coverage 目標に届かない場合は、未達理由と次に埋める Pekko contract / Integration / E2E scenario を `docs/plan/actor-test-pyramid.md` の follow-up 表に残す。private helper の枝葉で数字だけを埋めない
- [x] 8.8 テスト実行時間が悪化した場合、Integration / E2E 層の重複を削る。必要なら expensive test は個別 target に分ける

## Phase 9: 最終検証

- [x] 9.1 `./scripts/ci-check.sh ai all` を実行し、exit 0 を確認する
- [x] 9.2 `scripts/coverage.sh` の出力パスと actor coverage totals を PR 本文に記録する
  - 出力: `target/coverage/actor-test-pyramid/coverage.json`
  - totals: Function 86.74% / Line 85.35% / Region 84.72%
- [x] 9.3 Wave 1 coverage 目標の達成 / 未達理由 / follow-up を PR 本文に記録する
  - 目標達成。follow-up は `docs/plan/actor-test-pyramid.md` に記録済み。
- [x] 9.4 追加したテストが以下を満たすことを確認する:
  - no_std core と std adaptor の分離を壊していない
  - `Result` / `Option` / `#[must_use]` 戻り値を握りつぶしていない
  - 既存 helper と重複する大きな fixture を作っていない
  - Pekko reference / gap ID / public API contract のいずれかへ辿れる
- [x] 9.5 `openspec validate actor-test-pyramid --strict` またはローカル互換 CLI の status 確認で artifact 整合を確認する

## Phase 10: テストピラミッド網羅性ゲート

以下は Wave 1 では未完了。これらが完了するまでは、Contract / Integration / E2E の網羅性が担保されたとは扱わない。

- [ ] 10.1 Contract coverage matrix を作り、Pekko 代表 Spec ごとに `covered` / `deferred` / `not covered` を記録する:
  - classic: lifecycle, mailbox, death watch, receive timeout, scheduler / timer, FSM
  - typed: behavior, actor context, watch, supervision, timer, ask / pipeToSelf, event stream
  - typed testkit: actor testkit, behavior testkit, test probe
- [ ] 10.2 Contract 層で、各 `covered` 項目が Rust public API または public state machine から検証されていることを確認する。内部 helper の枝葉だけで coverage を上げた項目は `covered` にしない
- [ ] 10.3 Integration 層で、classic actor system / typed actor system / std adaptor の代表的な module 接続がそれぞれ少なくとも 1 件ずつ検証されていることを確認する
- [ ] 10.4 E2E 層で classic user flow を実装する:
  - system 起動
  - named child spawn
  - tell / ask
  - watch
  - stop
  - terminated / dead letter 観測
- [ ] 10.5 E2E 層で typed user flow を実装する:
  - typed system 起動
  - spawn
  - message adapter
  - ask / pipeToSelf
  - stop
  - signal 観測
- [ ] 10.6 E2E 層で std adaptor boot flow を実装する:
  - std actor system config
  - dispatcher / mailbox / scheduler / logging の実配線
  - graceful terminate
- [ ] 10.7 E2E / Integration が実時間 sleep に依存していないことを確認し、必要な場合は deterministic probe / manual tick / start_paused へ置き換える
- [ ] 10.8 Phase 10 が完了するまで、PR 本文や docs で「テストピラミッドの網羅性達成」と表現しない。表現は「Wave 1 coverage 目標達成」と「次 wave の網羅性ゲート定義」に留める
