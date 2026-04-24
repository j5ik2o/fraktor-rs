# Design: actor-test-pyramid

## Overview

本 change は actor 実装そのものではなく、`modules/actor-core` / `modules/actor-adaptor-std` のテスト構造を Pekko 契約中心に再整理する。目的は coverage の数値を上げることではなく、Pekko `actor` / `actor-typed` から翻訳したユーザー可視契約が、適切な粒度のテストで継続的に守られる状態を作ることである。

テストピラミッドは以下の 4 層とする。

| 層 | 主な配置 | 目的 | 実行コスト |
|----|----------|------|------------|
| Unit | `modules/actor-core/src/**/tests.rs` / `foo/tests.rs` | 型・純粋ロジック・状態機械の不変条件を固定する | 低 |
| Contract | `modules/actor-core/src/**/tests.rs` / `modules/actor-core/tests/*_contract.rs` | Pekko 由来の公開 API / セマンティクスを Rust API で固定する | 低〜中 |
| Integration | `modules/actor-core/tests/*.rs` / `modules/actor-adaptor-std/tests/*.rs` | actor system / typed system / std adaptor の実配線を検証する | 中 |
| Conformance / Regression | `modules/actor-core/tests/pekko_*.rs` / 既存 regression tests | gap-analysis ID や過去差分を再発させない | 中〜高 |

実装順は「目録 → helper 整理 → contract test → integration test → coverage 確認」とする。coverage は最後に使う。先に数値だけを追うと、leaf の枝葉を埋める作業に偏り、actor の本質である配送・監視・停止・再起動・typed behavior の契約漏れを見落としやすいためである。

## Detailed Design

### 1. Pekko 参照の扱い

実装フェーズでは、可能な環境で以下を再確認する。

- classic: `references/pekko/actor/src/main/scala/org/apache/pekko/`
- typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`
- classic tests: `references/pekko/actor-tests/src/test/scala/org/apache/pekko/actor/`
- typed tests: `references/pekko/actor-typed-tests/src/test/scala/org/apache/pekko/actor/typed/`
- typed testkit: `references/pekko/actor-testkit-typed/src/test/scala/org/apache/pekko/actor/testkit/typed/`
- 既存根拠: `docs/gap-analysis/actor-gap-analysis-evidence.md`
- 既存 change: `openspec/changes/*/proposal.md` / `design.md` / `specs/**/spec.md`

参照対象は「Scala のテストケース名を Rust に直訳する」ではなく、「Pekko が利用者に保証する観測可能な contract」に落とす。例えば dispatcher の内部実装差は Rust 側の `Dispatchers::resolve` / `canonical_id` の契約で検証し、ScalaTest の scheduler timing そのものは移植しない。

最初に参照する代表テストは以下とする。

- classic lifecycle / mailbox / death watch: `ActorLifeCycleSpec.scala`, `ActorMailboxSpec.scala`, `DeathWatchSpec.scala`, `ReceiveTimeoutSpec.scala`
- classic FSM / scheduler: `FSMActorSpec.scala`, `FSMTimingSpec.scala`, `FSMTransitionSpec.scala`, `SchedulerSpec.scala`, `TimerSpec.scala`
- typed core: `BehaviorSpec.scala`, `ActorContextSpec.scala`, `WatchSpec.scala`, `SupervisionSpec.scala`, `TimerSpec.scala`, `AskSpec.scala`
- typed event / testkit: `EventStreamSpec.scala`, `ActorTestKitSpec.scala`, `BehaviorTestKitSpec.scala`, `TestProbeSpec.scala`

submodule が読めない環境では、`docs/gap-analysis/actor-gap-analysis-evidence.md` の根拠と既存 openspec の参照行を一次入力として作業し、Pekko 実ファイル確認タスクを未完了のまま残す。

### 2. Unit 層

Unit 層は既存の `foo.rs` + `foo/tests.rs` / `tests.rs` 並置パターンを維持する。対象は次の通り。

- actor path / address / URI parser などの純粋変換
- mailbox queue / dispatcher registry / scheduler wheel などの局所状態機械
- typed `Behavior` / `Signal` / `Receptionist` などの値レベル契約
- supervision / restart limit / FSM transition など、外部 runtime なしに検証できる契約

追加基準:

- public 型ごとに「Pekko 互換の境界値」がある場合は Unit に置く。
- `std` 時間・tokio・thread を必要とする場合は Unit に置かず Integration 層へ送る。
- property test は、既に `scheduler` で使っているように小さな純粋ロジックに限定する。

### 3. Contract 層

Contract 層は、Pekko の public contract を Rust public API から検証する。テスト名またはコメントに対応 ID を入れる。

命名例:

- `pekko_dispatcher_default_id_contract`
- `pekko_mailbox_overflow_dead_letter_contract`
- `pekko_receive_timeout_not_influence_contract`
- `pekko_typed_behaviors_unhandled_contract`
- `pekko_fsm_named_timer_restart_contract`

配置方針:

- crate 内部型に触る必要がある contract は該当 module の `tests.rs` に置く。
- 外部 crate から見える公開面を検証する contract は `modules/actor-core/tests/*_contract.rs` に置く。
- std adaptor を必要とする contract は `modules/actor-adaptor-std/tests/*_contract.rs` に置く。

対象カテゴリ:

| カテゴリ | Contract 例 |
|----------|-------------|
| mailbox / dispatcher | enqueue void-on-success、suspend 中 enqueue、throughput deadline、Pekko primary id、alias chain |
| lifecycle / supervision | preRestart / postRestart、restart statistics、panic guard、post_stop ordering |
| death watch | duplicate watchWith、terminated dedup、local watch / unwatch |
| event / logging | dead letter marker、EventStream subchannel、logging filter |
| scheduler / timers | stale timeout discard、typed timer lifecycle、FSM named timer generation |
| typed | `Behaviors.same` / `unhandled` / `stopped` / `withTimers` / ask / pipeToSelf |
| routing / receptionist | group / pool routee selection、listing dedup、consistent hashing key precedence |

### 4. Integration 層

Integration 層は「複数 module の接続が正しいか」を検証する。数は絞る。

追加候補:

- classic ping-pong に watch / stop / dead letter 観測を含める。
- typed actor system で spawn → message adapter → ask / pipeToSelf → stop を通す。
- dispatcher / mailbox config を std adaptor 経由で組み立て、actor cell まで届くことを検証する。
- EventStream と LoggingAdapter の publish / subscribe / filter を actor system 経由で検証する。
- scheduler / timer は manual tick が使える箇所を優先し、実時間 sleep は既存 `check-unit-sleep` 方針に従って避ける。

Integration 層でしか見つからない漏れに絞り、Unit / Contract で済むものを system 起動テストにしない。

本 change の実装範囲は Wave 1 に限定する。Wave 1 は Contract 最大 5 件、Integration 最大 2 件とし、残りは `docs/plan/actor-test-pyramid.md` の follow-up 表に残す。これによりテストピラミッド全体を一度に完成させるのではなく、継続的に拡張できる土台を先に確立する。

### 5. Conformance / Regression 層

gap-analysis で一度検出した差分は、少なくとも 1 件の regression test に紐づける。

ルール:

- テスト名またはコメントに gap ID を残す。例: `AC-M4a`, `MB-H1`, `FS-M2`。
- done 化済み項目の再発防止テストを優先する。
- deferred の `AC-M4b` は remote / cluster 依存として本 change では pending inventory に残す。
- 既存 ignored test は棚卸しし、「今も仕様上 pending なのか」「実装済みだが ignore が残っているのか」を分類する。

ただし、本 change で全 done 項目を一気に埋めることはしない。Wave 1 に選んだ contract の regression を実装対象とし、他の gap ID は計画ドキュメントに次回以降の候補として残す。

### 6. Fixture / helper 方針

便利な `ActorTestManager` のような大きな helper は作らない。責務別に小さい support module を置く。

既存の `modules/actor-core/tests/fixtures/` は `kernel_public_surface.rs` が `include_str!` で読む compile fixture 置き場であり、共有 helper module としては使わない。compile fixture と runtime probe helper を混在させると import 経路が曖昧になるため、本 change では以下のように分離する。

- compile fixture: 既存どおり `modules/actor-core/tests/fixtures/**` に置く。
- runtime probe helper: `modules/actor-core/tests/support/mod.rs` から明示的に module wiring する。
- std adaptor helper: `modules/actor-adaptor-std/tests/support/mod.rs` から明示的に module wiring する。

候補:

- `modules/actor-core/tests/support/classic_probe.rs`: sender / receiver / dead letter の観測補助
- `modules/actor-core/tests/support/typed_probe.rs`: typed message / signal の観測補助
- `modules/actor-core/tests/support/manual_time.rs`: manual tick / scheduler driver の補助
- `modules/actor-adaptor-std/tests/support/std_system.rs`: tokio executor / std logging を含む最小 system 構築

ただし、実装前に既存 fixture / helper を確認し、重複する helper は増やさない。公開型を追加する場合は 1file1type 原則に従う。helper は integration test crate 内限定とし、production API へ漏らさない。

### 7. Coverage の使い方

`scripts/coverage.sh` が生成する HTML を使い、低 coverage ファイルを次の優先順位で読む。

1. Pekko contract に直結するがテストが薄い箇所
2. gap-analysis done 項目なのに regression が薄い箇所
3. public API だが external crate からの compile contract がない箇所
4. private helper の枝葉

coverage の現行 baseline は `scripts/coverage.sh` の直近実行結果で Function 83.79% / Line 83.36% / Region 82.74% である。Wave 1 の目標は Function 85% / Line 85% / Region 84% とする。

この目標は PR の受け入れ基準には含めるが、まだ CI gate にはしない。高価値な Pekko contract を優先した結果として一部の数値が届かない場合は、未達理由と次に埋める低 coverage contract を `docs/plan/actor-test-pyramid.md` の follow-up 表に残す。長期目標は Function / Line 90% 以上とし、coverage が複数回安定してから別 change で CI 閾値導入を検討する。

### 8. Validation

段階ごとに以下を実行する。

- `rtk cargo test -p fraktor-actor-core-rs --lib`
- `rtk cargo test -p fraktor-actor-core-rs --tests`
- `rtk cargo test -p fraktor-actor-adaptor-std-rs --features test-support`
- `rtk scripts/coverage.sh`
- 最終: `./scripts/ci-check.sh ai all`

`ci-check.sh` は cargo を内部実行するため、他の cargo 実行と並列にしない。

## Alternatives Considered

### Alternative 1: coverage が低い順にテストを足す

却下。短期的に line coverage は上がるが、actor の重要契約である mailbox / lifecycle / supervision / typed behavior の横断漏れを見つけにくい。

### Alternative 2: Pekko Scala tests を機械的に移植する

却下。ScalaTest / JVM thread / reflection / TestKit 前提を Rust に持ち込むと、no_std core と std adaptor 分離を壊しやすい。Pekko の観測可能 contract に翻訳する。

### Alternative 3: integration test を厚くする

却下。actor system 起動テストは価値が高いが遅く壊れやすい。ピラミッドとしては Unit / Contract を厚くし、Integration は接続確認に絞る。

### Alternative 4: test helper を production API として整備する

却下。正式リリース前であっても、helper が production API に漏れると後で削りにくい。必要な観測補助は test crate / test-support feature に閉じる。
