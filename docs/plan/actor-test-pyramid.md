# actor-test-pyramid 計画

## 目的

`modules/actor-core` / `modules/actor-adaptor-std` のテストを、Pekko 互換 contract を中心に Unit / Contract / Integration / E2E の 4 層へ整理する。coverage は最終確認の指標として使うが、private helper の枝葉を埋めるためではなく、Pekko 由来の観測可能 contract が薄い箇所を見つける入口として扱う。

## 参照

`references/pekko` submodule は `2dc8960074bfe269da1686609eb88663cb50ad8b` で確認した。

| 種別 | 参照パス |
|------|----------|
| classic 実装 | `references/pekko/actor/src/main/scala/org/apache/pekko/` |
| typed 実装 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` |
| classic tests | `references/pekko/actor-tests/src/test/scala/org/apache/pekko/actor/` |
| typed tests | `references/pekko/actor-typed-tests/src/test/scala/org/apache/pekko/actor/typed/` |
| typed testkit | `references/pekko/actor-testkit-typed/src/test/scala/org/apache/pekko/actor/testkit/typed/` |
| gap 根拠 | `docs/gap-analysis/actor-gap-analysis-evidence.md` |

## 分類ルール

| 層 | 配置 | 判断基準 |
|----|------|----------|
| Unit | `modules/actor-core/src/**/tests.rs` / `foo/tests.rs` | 型単位、純粋変換、状態機械、不変条件。system 起動を不要にできるものをここへ置く。 |
| Contract | `modules/actor-core/src/**/tests.rs` / `modules/actor-core/tests/*_contract.rs` / `modules/actor-adaptor-std/tests/*_contract.rs` | Pekko の public contract または fraktor-rs の公開 API 境界を Rust API から直接検証する。 |
| Integration | `modules/actor-core/tests/*.rs` / `modules/actor-adaptor-std/tests/*.rs` | actor system、dispatcher、mailbox、event stream、std adaptor の接続漏れを検出する。 |
| E2E | `modules/actor-core/tests/*_e2e.rs` / user-flow scenario tests | public API だけを使い、spawn → send / ask → watch → stop → terminate → observable event のようなユーザー操作に近い流れを検証する。 |

Conformance / Regression はピラミッドの実行層ではなく横断タグとする。gap-analysis ID や過去 change の再発防止テストは、Unit / Contract / Integration / E2E のいずれかに置き、テスト名またはコメントから ID を辿れるようにする。

## 既存目録

| 項目 | 現状 |
|------|------|
| Unit 並置テスト | `modules/actor-core/src/core/kernel` / `modules/actor-core/src/core/typed` / `modules/actor-adaptor-std/src` 配下で `tests.rs` が 53 件 |
| actor-core integration | `actor_path_e2e`, `byte_string`, `death_watch`, `event_stream`, `invoke_guard`, `kernel_public_surface`, `logging_filter_public_surface`, `ping_pong`, `supervisor`, `system_events`, `system_lifecycle`, `typed_scheduler` |
| actor-adaptor-std integration | `circuit_breakers_registry`, `dispatcher_public_surface`, `sp_h1_5_panic_guard`, `sp_h1_5_system_escalation` |
| E2E 候補 | `actor_path_e2e`, `ping_pong`, `supervisor`, `system_lifecycle`, `system_events`, `sp_h1_5_system_escalation`。ただし classic / typed の代表 user flow としてはまだ薄い。 |
| ignored test | `actor_cell/tests.rs` の 2 件。どちらも Phase A3 の terminate / finish_terminate 依存であり、Wave 1 では pending のまま残す。 |

## Fixture / Support 配置

`modules/actor-core/tests/fixtures/**` は `kernel_public_surface.rs` が `include_str!` で読む compile fixture 置き場のまま維持する。runtime probe helper と混在させない。

runtime helper が必要になった場合は以下に分ける。ただし Wave 1 では既存テスト内の小さい fixture で足りるため、新規 support module は作らない。

| 用途 | 配置 |
|------|------|
| actor-core runtime helper | `modules/actor-core/tests/support/mod.rs` から明示的に module wiring |
| actor-adaptor-std helper | `modules/actor-adaptor-std/tests/support/mod.rs` から明示的に module wiring |
| compile fixture | `modules/actor-core/tests/fixtures/**` |

helper 名には `Manager` / `Util` / `Service` / `Runtime` / `Engine` を使わない。std / tokio / thread / real time を使う helper は `actor-core` production code に入れない。

## Coverage Baseline と目標

`scripts/coverage.sh --format json --output target/coverage/actor-test-pyramid` の baseline は以下。

`scripts/coverage.sh` は actor 系 package の `lib` / `bins` と `tests` / `examples` を分割実行し、Unit / Contract / Integration / E2E のプロファイルを1つの report に統合する。

| 指標 | baseline | Wave 1 実測 | Wave 1 目標 | 長期目標 |
|------|----------|-------------|-------------|----------|
| Function | 83.79% | 86.78% | 85% | 90% 以上 |
| Line | 83.36% | 85.37% | 85% | 90% 以上 |
| Region | 82.74% | 84.74% | 84% | 90% 以上 |

低 coverage の上位は `actor_cell.rs`, `actor_context.rs`, `actor_path/*`, `actor-adaptor-std` の executor factory / std system config だった。Wave 1 では coverage 順ではなく、Pekko contract と公開境界へ直結する箇所を優先する。

Wave 1 実測は `scripts/coverage.sh --format json --output target/coverage/actor-test-pyramid` で生成した `target/coverage/actor-test-pyramid/coverage.json` の totals。Function / Line / Region の Wave 1 目標はすべて達成済み。次 wave では、数値だけを追わず `actor_cell.rs` の lifecycle contract と typed ask / pipeToSelf を優先する。

## Pekko 対応表

| Pekko Spec | fraktor-rs 既存 / Wave 1 対応 |
|------------|-------------------------------|
| `ActorLifeCycleSpec.scala` | 既存 `system_lifecycle`, `supervisor`, `actor_cell/tests.rs`。Wave 1 では対象外。 |
| `ActorMailboxSpec.scala` | 既存 `ping_pong::tell_respects_mailbox_backpressure`, mailbox unit。Wave 1 では actor path / std dispatcher 接続を優先。 |
| `DeathWatchSpec.scala` | 既存 `death_watch.rs`, `watchWith` regression。AC-M4b は remote / cluster 依存で pending。 |
| `ReceiveTimeoutSpec.scala` | 既存 `actor_context/tests.rs` receive-timeout 群。Wave 1 では対象外。 |
| `FSMActorSpec.scala` / `FSMTimingSpec.scala` / `FSMTransitionSpec.scala` | 既存 `fsm/tests.rs`。FS-M1 / FS-M2 の ID 紐づけは follow-up。 |
| `SchedulerSpec.scala` / `TimerSpec.scala` | 既存 scheduler / typed_scheduler。Wave 1 では対象外。 |
| typed `BehaviorSpec.scala` | typed behavior unit を follow-up で整理。 |
| typed `ActorContextSpec.scala` / `WatchSpec.scala` / `SupervisionSpec.scala` | 既存 typed / classic watch と supervision tests。Wave 1 では対象外。 |
| typed `AskSpec.scala` | ask / pipeToSelf は follow-up。 |
| typed `EventStreamSpec.scala` | EventStream subchannel は follow-up。 |
| typed testkit specs | testkit の専用 change で扱う。 |

## Wave 1 Scope

Wave 1 は Contract 最大 5 件、Integration 最大 2 件に限定する。E2E は既存候補の棚卸しまでとし、新規 E2E scenario は次 wave の受け入れ条件へ送る。

Phase 10 では Wave 1 後に残っていた網羅性ゲートを閉じるため、Contract matrix を `docs/plan/actor-contract-coverage-matrix.md` に分離し、classic / typed / std adaptor の代表 E2E を追加した。

| 種別 | 対象 | 理由 |
|------|------|------|
| Contract | actor path error display | public error の観測可能文字列と variant を固定し、0% file を価値のある境界で埋める。 |
| Contract | path resolution error display | registry / remote 経路の public error 境界を固定する。 |
| Contract | actor ref sender scheduled outcome | `ActorRefSender::apply_outcome` の default contract を固定する。 |
| Contract | std public dispatcher factory compile surface | `PinnedExecutor` / `PinnedExecutorFactory` を外部 crate から見える公開面として固定する。 |
| Integration | std dispatcher factories create executors and accept tasks | std adaptor から core `ExecutorFactory` へ接続できることを実行時に確認する。 |
| Integration | `std_actor_system_config` installs mailbox clock | std adaptor の production config が mailbox deadline clock を core config へ配線することを確認する。 |
| E2E | existing E2E inventory only | Wave 1 は Unit / Contract 寄りに coverage を上げたため、新規 E2E は入れない。既存 E2E 相当テストと不足 user flow を明示し、次 wave へ送る。 |

## Follow-up

| 候補 | 理由 |
|------|------|
| classic E2E user flow | Phase 10 で `modules/actor-core/tests/classic_user_flow_e2e.rs` として追加済み。 |
| typed E2E user flow | Phase 10 で `modules/actor-core/tests/typed_user_flow_e2e.rs` として追加済み。 |
| std adaptor E2E boot flow | Phase 10 で `modules/actor-adaptor-std/tests/std_adaptor_boot_e2e.rs` として追加済み。 |
| typed `Behaviors.same` / `unhandled` / `stopped` | typed contract 層として価値が高いが、Wave 1 では std 接続と public error 境界を優先。 |
| typed `with_timers` | 既存 scheduler tests との重複整理が必要。 |
| ask / pipeToSelf | actor_context には coverage があるが、typed 側の public contract 対応表が薄い。 |
| EventStream subchannel / dead letter marker | Unit は厚いが system 経由の contract 対応表を別 wave で整理する。 |
| FS-M1 / FS-M2 ID 紐づけ | 既存 FSM tests へ雑にコメントを増やさず、対象 test 名と gap ID を見直す。 |
| AC-M4b | remote / cluster 依存のため actor-core 単体では無理に再現しない。 |
| `actor_cell.rs` の未到達行 | 重要度は高いが、lifecycle / restart の大きな scope になりやすいため専用 wave に分ける。 |
| `actor_context.rs` の残り public contract | `reply` / `forward` / timer / stash は厚いが、coverage 上はまだ 84.85% / 82.63% に残るため、Pekko `ActorContextSpec.scala` と対応を再確認する。 |

Wave 1 追加後の coverage 目標は達成済み。次 wave でも private helper の枝葉で数字だけを埋めず、この follow-up 表から次の Pekko contract を選ぶ。
