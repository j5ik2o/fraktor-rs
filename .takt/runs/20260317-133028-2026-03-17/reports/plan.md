# タスク計画

## 元の要求
Implement using only the files in `.takt/runs/20260317-133028-2026-03-17/context/task`.
Primary spec: `.takt/runs/20260317-133028-2026-03-17/context/task/order.md`.
Use report files in Report Directory as primary execution history.
Do not rely on previous response or conversation summary.

## 分析結果

### 目的
repo 全体の時間依存テスト再設計のうち、現在ワークツリーに残っている未完了差分を埋める。
具体的には、単体テストから壁時計待ちを排除し、統合テストは実時間のまま残しつつ固定 sleep を状態待ちへ置換し、`scripts/ci-check.sh` を unit-fast / integration / long-running に分離する。

### 参照資料の調査結果（参照資料がある場合）
参照資料は `order.md` のみだった。加えて、実行履歴として同一 Report Directory 内の `plan.md`、`test-scope.md`、`test-decisions.md` を確認した。

現状との差分は次の通り。
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs` と `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` は既に `FakeClock` と `new_with_clock(...)` 前提へ変更済みだが、本体の `modules/actor/src/std/pattern/circuit_breaker.rs` にはその注入点がなく、`Instant::now()` / `elapsed()` 直結のまま。
- `modules/actor/src/std/scheduler/tick/tests.rs`、`modules/remote/tests/quickstart.rs`、`modules/remote/tests/multi_node_scenario_integration.rs`、`modules/remote/src/std/endpoint_transport_bridge/tests.rs` の一部は `start_paused = true` に寄っているが、`modules/actor/Cargo.toml` と `modules/remote/Cargo.toml` の dev-dependencies に `tokio/test-util` が未追加。
- `modules/actor/src/core/system/base/tests.rs` は関連ケースに `sleep` が残っておらず、今回の追加対応は不要。
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs` には test double 内の `thread::sleep`、統合寄りケースの固定 `tokio::time::sleep`、paused time 前進後の `yield_now()` 不足が残っている。
- `modules/remote/src/std/transport/tokio_tcp/tests.rs` と `modules/cluster/src/std/tokio_gossip_transport/tests.rs` は実 transport / 実 runtime 統合テストとして残す方針に合うが、固定 sleep 待ちのまま。
- `scripts/ci-check.sh` は `test` / `all` しかなく、テスト階層分離と sleep 禁止の軽量検査が未実装。

### スコープ
- `modules/actor/src/std/pattern/circuit_breaker.rs`
- `modules/actor/src/std/pattern/circuit_breaker_shared.rs`
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs`
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs`
- `modules/actor/src/std/scheduler/tick/tests.rs`
- `modules/actor/Cargo.toml`
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs`
- `modules/remote/tests/quickstart.rs`
- `modules/remote/tests/multi_node_scenario_integration.rs`
- `modules/remote/src/std/transport/tokio_tcp/tests.rs`
- `modules/remote/Cargo.toml`
- `modules/cluster/src/std/tokio_gossip_transport/tests.rs`
- `scripts/ci-check.sh`

追加対応不要と判断したもの。
- `modules/actor/src/core/system/base/tests.rs`: 関連テストは即時 assert 構成で、壁時計待ちが残っていないため不要。

### 検討したアプローチ（設計判断がある場合）
| アプローチ | 採否 | 理由 |
|-----------|------|------|
| `CircuitBreaker` に公開 `Clock` trait を追加する | 不採用 | 公開 API を増やし過ぎる。用途はテスト限定で YAGNI に反する |
| `CircuitBreaker` に非公開の now provider 注入点を追加する | 採用 | 根本原因である `Instant::now()` 直結だけを局所的に解消できる |
| paused Tokio time を使うテストは test 側だけ変更する | 不採用 | `tokio/test-util` 未追加のままではコンパイルできず、実装が閉じない |
| `endpoint_transport_bridge` の全テストを物理的に integration へ移す | 不採用 | private API 依存が強く、今回のスコープでは構造変更コストが高い |
| `endpoint_transport_bridge` の統合寄りケースだけ feature gate / CI レーンで分離する | 採用 | 既存構造を大きく壊さずに fast unit と integration を分けられる |
| 実 transport テストを fake/manual time に寄せる | 不採用 | `order.md` のグループC方針に反する。契約確認として実時間統合のまま残すべき |
| 実 transport テストの固定 sleep を bounded poll / condition wait に変える | 採用 | 実時間統合を維持しつつ待機時間の無駄を減らせる |

### 実装アプローチ
`CircuitBreaker` は内部に now provider を持てる構造へ最小変更し、`new()` は既存のまま、テスト専用の `new_with_clock(...)` を追加する。`transition_to_open`、Open 判定、remaining 計算をすべてその provider 経由へ揃え、`CircuitBreakerShared` はその注入点を委譲するだけにする。これで、既に追加済みの `FakeClock` ベーステストを成立させる。

`CircuitBreakerShared` の cancel safety テストは、`timeout(1ms)` + `sleep(60s)` をやめて、未完了 future を `spawn` して `abort` / drop する構成に置き換える。これにより、単体テストから実時間待ちを除去する。

paused time へ移行済みの `scheduler/tick`、`quickstart`、`multi_node`、`endpoint_transport_bridge` の timer 系テストは、`tokio/test-util` を `modules/actor/Cargo.toml` と `modules/remote/Cargo.toml` に追加したうえで、必要箇所に `tokio::task::yield_now().await` を入れて spawned task の進行を保証する。

`endpoint_transport_bridge` の test double 内 `thread::sleep` は、atomicity を見たいケースでは `Notify` / barrier 相当の同期へ置き換えて deterministic にする。統合寄りケースは fast unit から外す方針を取りつつ、固定 80ms/200ms 待ちは状態到達待ちへ寄せる。少なくとも `queue_is_full` のようなケースは bounded wait にする。

`tokio_tcp` と `tokio_gossip_transport` は実時間統合テストのまま残し、listener 起動や UDP 受信を固定 sleep ではなく bounded poll に置換する。これによりグループC方針を守りつつ、不要な待機を削減する。

`scripts/ci-check.sh` は `unit-fast`、`integration`、`long-running` を追加し、`all` は `unit-fast` を先に通す full 経路にする。unit-fast では grep ベースで `thread::sleep` / `tokio::time::sleep` を禁止し、integration allowlist は明示的に分離した対象のみに限定する。

## 実装ガイドライン（設計が必要な場合のみ）
- `CircuitBreaker` の時間制御は公開型を増やさず、非公開注入点で閉じること。`new()` の契約は維持すること。
- `CircuitBreakerShared` はラッパー責務に徹し、時間制御ロジックを再実装しないこと。
- 既存の deterministic パターンとして `modules/streams/examples/std_materializer_support.rs` の manual tick 駆動と `modules/remote/examples/loopback_quickstart/main.rs` の `pump_manual_drivers` を参照すること。
- 状態到達待ちの既存パターンとして `modules/actor/tests/event_stream.rs` や `modules/actor/src/core/actor/actor_context/tests.rs` の `wait_until` を参照すること。
- paused time を使うテストでは、仮想時間前進後に spawned task の実行が必要かを確認し、必要な場所だけ `yield_now()` を追加すること。
- `scripts/ci-check.sh` の変更では `usage()`、実行関数、`all`、`main()` の case dispatch を一括で更新すること。サブコマンドだけ増やして help / dispatch を更新し忘れないこと。
- `sleep` を短い `sleep` や `timeout` に置き換えるだけの修正は禁止。論理時間化か状態到達待ちへ変えること。
- 今回の変更で不要になる旧 sleep helper、未使用 import、未使用コメントは同時に削除すること。

## スコープ外（項目がある場合のみ）
| 項目 | 除外理由 |
|------|---------|
| `modules/actor/src/core/dispatch/dispatcher/tests.rs` など、`order.md` に列挙されていない他の sleep 使用箇所 | 同じパターンは存在するが、今回の主対象 A/B/C に含まれていない |
| examples の待機削減 | `order.md` はテスト階層と CI 導線の再設計が主題であり、example 実行時間の改善は要求外 |
| `modules/cluster/Cargo.toml` への `tokio/test-util` 追加 | cluster 側は paused time 化ではなく、実時間統合テストの bounded poll 化で対応可能なため不要 |

## 確認事項（あれば）
- なし