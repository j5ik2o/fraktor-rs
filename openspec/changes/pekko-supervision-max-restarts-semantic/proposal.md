## Why

`modules/actor-core/src/core/kernel/actor/supervision/base.rs:30,137-139` の `SupervisorStrategy::max_restarts: u32` は **`== 0 ⇒ 無制限`** という Pekko と真逆の意味を採用している。Pekko の `FaultHandling.scala` (および `OneForOneStrategy` / `AllForOneStrategy`) は `maxNrOfRetries = -1 ⇒ 無制限 / = 0 ⇒ 即 Stop / > 0 ⇒ 最大 n 回` を契約としており、ユーザが `application.conf` / Pekko サンプルコードの設定値をそのまま流用すると **暗黙に restart 契約が反転する** 落とし穴になる（docs/gap-analysis/actor-gap-analysis.md SP-M1）。typed 層 (`restart_supervisor_strategy.rs:48-59`) は `with_limit(i32)` の `-1` マジック値で部分的に API 側を救済しているが、kernel 層の表現自体が Pekko と乖離しており、kernel API を直接使う経路（`MessageInvokerPipeline` 配線、将来の persistence / cluster 側 supervision、テストサポート）に反転した意味が露出する。これを残しておく限り Pekko parity の下層契約を満たさない。正式リリース前の今こそ kernel 型表現を Pekko 直訳へ差し替える最適タイミングである。

## What Changes

- **BREAKING**: kernel `SupervisorStrategy::max_restarts` のフィールド型を `u32` から **`Option<u32>` (`None ⇒ 無制限` / `Some(0) ⇒ retry なし (即 Stop)` / `Some(n) ⇒ 最大 n 回`)** へ変更する。関連するコンストラクタ (`new`, `default`, factory) と `handle_failure` の判定ロジック (`base.rs:137-139`) を Pekko の accumulator 契約 (`withinTimeRange` が `0` のとき全期間加算、それ以外は window 内のみカウント) と完全に一致させる。
- **BREAKING**: typed 層 `RestartSupervisorStrategy::with_limit(max_restarts: i32, within: Duration)` の `i32 + -1 マジック値 + 0 でのパニック` を廃止し、Pekko 直訳 API に差し替える。候補:
  - `with_limit(max_restarts: u32, within)`: 有限回数の指定 (0 を含む、`Some(n)` 相当)
  - `with_unlimited_restarts(within)`: 無制限の明示 (`None` 相当)
  - あるいは `Option<u32>` を直接引数に取る形
  - いずれの表現でも panic 経路を排除する。
- **BREAKING**: `BackoffSupervisorStrategy::max_restarts`, `BackoffSupervisorStrategyConfig`, および全テストを新表現に合わせて更新。
- **BREAKING**: `RestartStatistics` の内部状態を現状の **sliding window (`Vec<Duration>` + `prune` による window 外削除)** から、Pekko `ChildRestartStats.requestRestartPermission` / `retriesInWindowOkay` と同型の **one-shot window (restart count + window start time、window 超過で count=1 + window_start=now + 常に permit)** に差し替える。`record_failure(now, window, max_history) -> usize` を廃止し、`request_restart_permission(now, limit: RestartLimit, window: Duration) -> bool` に置換 (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala:56-86`)。
- `handle_failure` の directive 別 accumulator 契約を Pekko に一致させる (MUST):
  - `Restart + Unlimited` (and `window == ZERO`): 常に `Restart`、内部 count 更新なし
  - `Restart + Unlimited + window > 0`: Pekko `retriesInWindowOkay(retries=1, window)` 経路と一致 — 初回は permit + count=1 + window_start=now、window 内 2 回目 (`retriesDone=2 > retries=1`) で false 返却 ⇒ `handle_failure` が reset + Stop、window expire 時は count=1 + window_start=now + permit (Pekko quirk の直訳)。spec.md 「Unlimited + window > 0 は Pekko `retriesInWindowOkay(1, window)` 経路で window 内 2 回目以降は Stop へ昇格する」Scenario を正規契約とする
  - `Restart + WithinWindow(0)`: Pekko `(Some(0), _) => false` と一致し、**`record_failure` / `request_restart_permission` を呼ばずに即 Stop** (統計非更新)
  - `Restart + WithinWindow(n) + window == ZERO`: Pekko `(Some(n), None) => count += 1; count <= n` と一致
  - `Restart + WithinWindow(n) + window > 0`: Pekko one-shot window、window 内超過で Stop + reset、window expire で count=1 + window_start=now + `Restart`
  - `Stop` / `Escalate`: 統計リセット (`count=0, window_start=None`)
  - `Resume`: 統計を **変更しない** (Pekko `FaultHandling.scala` の `Resume` 分岐は `childStats` に触れないため)
- gap-analysis `SP-M1` 項目を `~~medium~~ done` にマークする。

## Capabilities

### New Capabilities

- `pekko-supervision-max-restarts-semantic`: Pekko の `maxNrOfRetries` 契約に一致する kernel 型表現を定義し、`SupervisorStrategy::handle_failure` の restart accumulator セマンティクス (`withinTimeRange` リセット条件、`Restart 以外の directive でのリセット`) を契約として固定する。typed 層 API が kernel 契約と矛盾しないことも要件に含める。

### Modified Capabilities

- なし（既存 `pekko-restart-completion` は 2 フェーズ state machine の contract で、`maxNrOfRetries` の semantic は扱っていないため修正対象外）。

## Impact

- **破壊される公開 API**:
  - kernel: `SupervisorStrategy::new(..., max_restarts: u32, ...)`, `max_restarts() -> u32`, `BackoffSupervisorStrategy::max_restarts`
  - typed: `RestartSupervisorStrategy::with_limit(i32, Duration)`, `BackoffSupervisorStrategyBuilder::with_max_restarts(u32)` のシグネチャ
- **影響するモジュール**:
  - `modules/actor-core/src/core/kernel/actor/supervision/{base.rs, backoff_supervisor_strategy.rs, supervisor_strategy_config.rs, restart_statistics.rs, restart_statistics/tests.rs}`
  - `modules/actor-core/src/core/typed/{restart_supervisor_strategy.rs, backoff_supervisor_strategy.rs, supervisor_strategy.rs, dsl/supervise.rs}` とそれらの `tests.rs`
  - `modules/actor-adaptor-std/` 側で supervision config を構築しているテスト / サンプル（存在すれば）
  - `remote` / `cluster` 相当モジュール: 現時点で fraktor-rs には未存在。将来当該モジュールが追加される際に `SupervisorStrategy` を構築する箇所があれば follow-up change で追従する (本 change では対象外)
- **後方互換**: 不要（CLAUDE.md「後方互換は不要、破壊的変更を恐れず最適設計」）。
- **ドキュメント**: `docs/gap-analysis/actor-gap-analysis.md` の SP-M1 行を done 化、残存 medium 件数を 13 → 12 に更新。
- **依存関係**: 新規依存なし（`NonZeroU32` / `Option<u32>` は `core` に含まれる）。
- **非対象**: AC-M3 (FailedFatally / isFailed ガード), AC-M5 (NotInfluenceReceiveTimeout マーカー) は別 change。SP-H1 decider 粒度は完了済み (`2026-04-21-2026-04-20-pekko-panic-guard`)。
- **非対象 (Pekko 側に存在するが本 change で扱わない項目)**:
  - `SupervisorStrategy.loggingEnabled` / `loggingLevel`: 既に `SupervisorStrategy::logging_enabled` / `log_level` フィールドが存在し、本 change の `maxNrOfRetries` 意味論とは直交するため対象外
  - Pekko `StoppingSupervisorStrategy` 相当: fraktor-rs kernel に対応する型は存在せず、`SupervisorStrategyKind` が `OneForOne` / `AllForOne` の 2 variant のみ。Stop は decider の返り値で表現するため別 strategy 型は導入しない
