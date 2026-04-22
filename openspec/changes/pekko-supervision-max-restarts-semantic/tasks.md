## 0. 準備

- [ ] 0.1 本 change の **実装開始前** の状態を確認する:
  - 最新の `main` に追従し、ブランチ `impl/pekko-supervision-max-restarts-semantic` を切る
  - `rtk cargo check -p fraktor-actor-core-rs` が通ることを確認 (**本 change 着手前時点で** warning 0。実装フェーズ中は型変更に伴う一時的な compile error / warning は許容し、各 Phase 末の再検証で回復していればよい)
  - `rtk cargo test -p fraktor-actor-core-rs --lib --no-run` が通ることを確認 (既存テストが compile する状態)
- [ ] 0.2 参照ファイルを一度読む:
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala`
    — `maxNrOfRetries` 契約と `handleFailure` の directive 別分岐
  - `modules/actor-core/src/core/kernel/actor/supervision/base.rs` (現状の反転実装)
  - `modules/actor-core/src/core/kernel/actor/supervision/restart_statistics.rs` (`max_history` 引数の混入)
  - `modules/actor-core/src/core/typed/restart_supervisor_strategy.rs:48-59` (i32 + -1 マジック)

## 1. kernel に `RestartLimit` enum を新設

- [ ] 1.1 `modules/actor-core/src/core/kernel/actor/supervision/restart_limit.rs` を新規作成:
  - `pub enum RestartLimit { Unlimited, WithinWindow(u32) }`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
  - rustdoc で Pekko `maxNrOfRetries = -1` / `= 0` / `> 0` との対応を明示
  - type-per-file dylint を満たすため 1 ファイル 1 型
- [ ] 1.2 `supervision/mod.rs` (または該当 mod 宣言ファイル) に `pub mod restart_limit;` と `pub use restart_limit::RestartLimit;` を追加
- [ ] 1.3 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 2. `SupervisorStrategy` と `handle_failure` の型と orchestration を差し替え

- [ ] 2.1 `modules/actor-core/src/core/kernel/actor/supervision/base.rs`:
  - フィールド `max_restarts: u32` → `max_restarts: RestartLimit`
  - `Debug` 実装の `max_restarts` フィールド出力を `RestartLimit` 対応に
  - `new(..., max_restarts: RestartLimit, ...)` / `default` / `with_*` コンストラクタの引数型を更新
  - `max_restarts()` getter の戻り値型を `RestartLimit` に変更
- [ ] 2.2 `handle_failure` の判定を design.md Decision 3 の orchestration パターンに書き換え:
  - `Restart`: `statistics.request_restart_permission(now, self.max_restarts, self.within)` を呼び、
    `true` なら `Restart`、`false` なら `statistics.reset()` + `Stop`
  - `Stop`: `statistics.reset()` の後 `Stop`
  - `Escalate`: `statistics.reset()` の後 `Escalate`
  - `Resume`: **`statistics` に一切触れずに** `Resume` のみ返す (Pekko `FaultHandling` の `Resume` 分岐参照)
  - 旧判定ロジック (`self.max_restarts == 0` / `count as u32 > self.max_restarts` / `Some(limit)` 構築 / `record_failure` 呼び出し) を完全に削除
- [ ] 2.3 rustdoc で `within: Duration::ZERO` が Pekko `Duration.Inf` に相当し「window なし」のセンチネルである旨を明記
- [ ] 2.4 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 3. `RestartStatistics` を Pekko one-shot window 実装に書き直す

- [ ] 3.1 `modules/actor-core/src/core/kernel/actor/supervision/restart_statistics.rs` の内部 state 置換:
  - `failures: Vec<Duration>` を削除
  - 代わりに `restart_count: u32` / `window_start: Option<Duration>` の 2 フィールドを持つ
  - `use alloc::vec::Vec` のインポートを削除 (不要化)
  - `#[derive(Clone, Debug, PartialEq, Eq, Default)]` を維持
- [ ] 3.2 公開 API の差し替え:
  - `record_failure(now, window, max_history) -> usize` を **削除**
  - `failures_within(window, now) -> usize` を **削除**
  - `failure_count() -> usize` を **削除** (または `restart_count()` にリネームしつつ戻り値 `u32` に)
  - `prune(window, now)` (private) を **削除**
  - `pub fn restart_count(&self) -> u32` を追加 (state accessor)
  - `pub fn window_start(&self) -> Option<Duration>` を追加 (state accessor)
  - `pub fn request_restart_permission(&mut self, now: Duration, limit: RestartLimit, window: Duration) -> bool` を追加 (Pekko `ChildRestartStats.requestRestartPermission` と行単位一致、design.md Decision 3 参照)
  - `fn retries_in_window_okay(&mut self, retries: u32, window: Duration, now: Duration) -> bool` を private helper として追加 (Pekko `FaultHandling.scala:64-86` 直訳)
  - `pub fn reset(&mut self)` は維持し、`restart_count = 0` + `window_start = None` に更新
- [ ] 3.3 `restart_statistics.rs` の rustdoc に Pekko `ChildRestartStats` / `retriesInWindowOkay` への行単位対応と `Duration::ZERO` センチネル意味を明記
- [ ] 3.4 `restart_statistics/tests.rs` を全面書き換え:
  - 旧 `record_failure` / `failures_within` / `prune` 前提のテストを **削除**
  - 新 Scenario (spec.md の `request_restart_permission` / `retries_in_window_okay` 節) に対応するテストを追加:
    - `(Unlimited, ZERO)` で counter 非更新 + 常に true
    - `(Unlimited, 10s)` で `retries_in_window_okay(1, 10s)` 経路: 初回は permit + count=1、window 内 2 回目は `retriesDone=2 > retries=1` で **false 返却** (Pekko quirk の直訳)、window expire 時は count=1 + window_start=now + true
    - `(Unlimited, 10s)` で window expire を挟んだ場合 (例: 0s permit → 15s permit) は再び permit され `restart_count == 1`、`window_start == Some(15s)` になる
    - `(WithinWindow(0), _)` で counter / window_start 非更新 + false
    - `(WithinWindow(3), ZERO)` で count++ で n まで true、n+1 で false
    - `(WithinWindow(3), 10s)` で window 内で count 進行 + n+1 目で false + window expire で count=1 + true
    - `reset()` で count と window_start 両方クリア
    - `saturating_add` 境界: `restart_count == u32::MAX - 1` の state で `WithinWindow(5), ZERO` を呼んでも overflow せず `u32::MAX` 止まり、`count <= 5 = false` が維持されること (境界テスト 1 件)
- [ ] 3.5 `rtk cargo test -p fraktor-actor-core-rs restart_statistics` 全 passing

## 4. `BackoffSupervisorStrategy` / `supervisor_strategy_config` の型と orchestration を更新

- [ ] 4.1 `modules/actor-core/src/core/kernel/actor/supervision/backoff_supervisor_strategy.rs`:
  - `max_restarts: u32` を `max_restarts: RestartLimit` に変更
  - `new` / `default` / builder (`max_restarts()`, `with_max_restarts()`) のシグネチャを `RestartLimit` ベースに差し替え
  - `handle_failure` 内部の上限判定を 2.2 と同じ orchestration (`request_restart_permission` → true/false 分岐) に合わせる
- [ ] 4.2 `modules/actor-core/src/core/kernel/actor/supervision/supervisor_strategy_config.rs`:
  - `supervisor_strategy_config.rs:59` 付近の反転判定 (`if max == 0 { None }` / `record_failure(now, reset_after, ...)`) を削除
  - 新 orchestration (`statistics.request_restart_permission(now, self.max_restarts, reset_after)` → bool) に差し替え
  - `RestartLimit` 型を受け取る形に公開 API を更新
- [ ] 4.3 `supervisor_strategy_config/tests.rs` の `SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), ...)` 呼び出しを
  `SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, RestartLimit::WithinWindow(3), Duration::from_secs(5), ...)` に更新
- [ ] 4.4 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 5. typed 層 API を Pekko 直訳 2 メソッドに差し替え

- [ ] 5.1 `modules/actor-core/src/core/typed/restart_supervisor_strategy.rs`:
  - `pub fn with_limit(self, max_restarts: i32, within: Duration) -> Self` を
    `pub fn with_limit(self, max_restarts: u32, within: Duration) -> Self` に差し替え
  - 本体から `if max_restarts == -1` / `assert!(max_restarts != 0, ...)` / `u32::try_from` を削除
  - 新メソッド `pub fn with_unlimited_restarts(self, within: Duration) -> Self` を追加
  - `max_restarts()` 公開 getter の戻り値型を `RestartLimit` に変更 (あるいは `u32` 返却を維持する場合は rustdoc で Pekko との対応を明記)
- [ ] 5.2 `modules/actor-core/src/core/typed/backoff_supervisor_strategy.rs`:
  - `pub fn with_max_restarts(mut self, max_restarts: u32) -> Self` を Pekko 契約の `u32`
    (有限 n、0 を含む) として継続使用しつつ、kernel 側 `RestartLimit::WithinWindow(n)` に変換
  - `pub fn with_unlimited_restarts(mut self) -> Self` を追加 (kernel 側 `RestartLimit::Unlimited` を設定)
  - `max_restarts()` 公開 getter の戻り値型を合わせる
- [ ] 5.3 `modules/actor-core/src/core/typed/dsl/supervise.rs`:
  - `.with_max_restarts(1)` などの既存呼び出しで Pekko 契約が守られるように `RestartLimit` への変換経路を追加
  - 必要に応じて DSL 側にも `with_unlimited_restarts()` ショートカットを提供
- [ ] 5.4 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 6. テストを新契約に合わせて書き換え + Pekko 契約 Scenario を追加

- [ ] 6.1 `modules/actor-core/src/core/kernel/actor/supervision/base/tests.rs` (または `base.rs` 内の `tests` モジュール):
  - `RestartLimit::Unlimited` + `within = ZERO` で `Restart` 連続、counter 非更新 (Pekko `(None, _)` arm)
  - `RestartLimit::Unlimited` + `within > 0` で Pekko `retries_in_window_okay(retries=1, window)` 経路: 初回 `window_start` 設定、window expire 時に `count=1` + `window_start=now` + permit
  - `RestartLimit::WithinWindow(0)` で `request_restart_permission` が counter 更新せず false、`handle_failure` が reset + `Stop`
  - `RestartLimit::WithinWindow(n)` + `within = ZERO` で counter 単調増加 → `count > n` で reset + `Stop`
  - `RestartLimit::WithinWindow(n)` + `within > 0` で window 内超過 → `Stop` + reset、window expire 時 → `count=1` + `window_start=now` + `Restart`
  - `decider = Stop` / `Escalate` で統計リセットのテスト
  - `decider = Resume` で `restart_count` / `window_start` が **一切変化しない** ことの assertion を含むテスト
- [ ] 6.2 `modules/actor-core/src/core/typed/restart_supervisor_strategy/tests.rs`:
  - 既存 `#[should_panic(expected = "max_restarts must be -1 or at least 1")]` 系テスト 2 件を **削除**
  - `with_limit(0, within)` が panic せず `RestartLimit::WithinWindow(0)` を構築するテストを追加
  - `with_limit(3, within)` / `with_unlimited_restarts(within)` の期待値を `RestartLimit` ベースに更新
- [ ] 6.3 `modules/actor-core/src/core/typed/backoff_supervisor_strategy/tests.rs`:
  - 既存 `assert_eq!(strategy.max_restarts(), 0)` などの「0 = unlimited」を暗黙前提にしたテストを
    新 contract (`RestartLimit::Unlimited` を直接確認) に書き換え
  - **意味逆転確認**: 各 `assert_eq!(max_restarts(), 0)` について、元テストのコンテキスト
    (テスト名 / セットアップ / 期待 directive) を読み、`0` が「unlimited」意図だったか
    「有限 0 回 = 即 Stop」意図だったかを判別する。前者なら `RestartLimit::Unlimited`、
    後者なら `RestartLimit::WithinWindow(0)` + Stop 発火確認テストに変換。両者を機械的に
    `RestartLimit::Unlimited` へ一括変換するのは **禁止** (意味逆転を覆い隠すため)
- [ ] 6.4 `modules/actor-core/src/core/typed/supervisor_strategy/tests.rs`:
  - `assert_eq!(strategy.max_restarts(), 0)` を `RestartLimit::Unlimited` 比較に変換
- [ ] 6.5 `modules/actor-core/src/core/typed/dsl/supervise/tests.rs`:
  - `with_max_restarts(1)` 等の呼び出し結果を新 contract で検証
- [ ] 6.6 `rtk cargo test -p fraktor-actor-core-rs supervision` 全 passing
- [ ] 6.7 `rtk cargo test -p fraktor-actor-core-rs` 全 passing (kernel 単体)

## 7. 暗黙の反転依存を grep で潰す

- [ ] 7.1 `rtk grep -rn "max_restarts == 0\|max_restarts() == 0" modules/` で 0 件 (反転判定の残留検出)
- [ ] 7.2 `rtk grep -rn "max_restarts must be -1 or at least 1" modules/` で 0 件 (panic メッセージ残留)
- [ ] 7.3 `rtk grep -rn "max_restarts: i32\|with_limit(-1" modules/` で 0 件 (i32 magic value 残留)
- [ ] 7.4 `rtk grep -rn "max_restarts: u32" modules/actor-core/` で **kernel 由来の参照が 0 件** であること (typed 側 `BackoffSupervisorStrategyBuilder::with_max_restarts(u32)` など、Pekko 契約の有限値引数として意図的に残すもののみ許容。残す場合はコメントで理由明記)

## 8. gap-analysis 更新

- [ ] 8.1 `docs/gap-analysis/actor-gap-analysis.md` SP-M1 行を更新:
  - 深刻度欄を `medium` → `~~medium~~ done` に変更
  - 閉塞 archive 名 (`2026-04-22-pekko-supervision-max-restarts-semantic` もしくは実アーカイブ日付) への参照を備考欄に追記
- [ ] 8.2 `docs/gap-analysis/actor-gap-analysis.md` まとめセクション:
  - 残存内部セマンティクス数値を `medium 13` → `medium 12` に更新
  - 第11版エントリを冒頭の更新履歴に追加 (分析日 / 本 change 完了の要約)
- [ ] 8.3 gap-analysis 更新を含めて PR を作成する前に `rtk grep -n "SP-M1" docs/gap-analysis/actor-gap-analysis.md` で done マーカーを確認

## 9. 参照実装との整合性検証 (Pekko との行単位突合)

- [ ] 9.1 `references/pekko/.../FaultHandling.scala` の以下 2 箇所と本 change の実装を行単位で突合:
  - `ChildRestartStats.requestRestartPermission` (L56-62) の 4 case arm と `RestartStatistics::request_restart_permission` の match 4 arm
  - `retriesInWindowOkay` (L64-86) の 1-shot window ロジック (windowStart 初回設定 / inside-window counter 更新 / outside-window reset + return true) と `RestartStatistics::retries_in_window_okay` の実装
  - `handleFailure` 相当処理の `Restart` / `Stop` / `Escalate` / `Resume` 各 directive での `childStats` 更新/非更新 (特に `Resume` が一切触れない点) と本 change の `handle_failure` orchestration
- [ ] 9.2 Pekko の `OneForOneStrategy` / `AllForOneStrategy` コンストラクタで
  `maxNrOfRetries: Int = -1` / `withinTimeRange: Duration = Duration.Inf` の default が
  fraktor-rs の `SupervisorStrategy::default()` / `with_unlimited_restarts(Duration::ZERO)` と意味的に一致することを確認
- [ ] 9.3 本 change で **新たに** Pekko 非互換を作っていないことを検証:
  - 既存の他テスト (`handle_watch` / `handle_recreate` / `handle_failure` を駆動するもの) が passing のまま
  - 本 change で `#[ignore]` が新規付与されていない

## 10. CI / lint の final ゲート

- [ ] 10.1 **OpenSpec artifact 整合性の検証**:
  `openspec validate pekko-supervision-max-restarts-semantic --strict`
  が valid を返すこと
- [ ] 10.2 `./scripts/ci-check.sh ai all` が exit 0
  - dylint 8 lint 全 pass: mod-file / module-wiring / type-per-file / tests-location / use-placement /
    rustdoc / cfg-std-forbid / ambiguous-suffix
  - cargo test / clippy / fmt が全て pass
  - **TAKT ルール: 本ゲートは change のマージ直前にのみ実行する**

## 11. PR 作成 / マージ / アーカイブ

- [ ] 11.1 `feat(actor-core): align supervisor maxNrOfRetries semantic with Pekko (SP-M1)`
  という題で PR を作成、本 change の change name をリンク
- [ ] 11.2 PR 本文に以下を含める:
  - Pekko `maxNrOfRetries` 契約との対応表
  - 破壊的 API 変更の一覧 (`with_limit(i32)` → `with_limit(u32)` + `with_unlimited_restarts()`)
  - `RestartLimit` 導入による enum 化の設計根拠 (design.md Decision 1 の要約)
  - gap-analysis SP-M1 done 化の反映
- [ ] 11.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 11.4 マージ後、別 PR で change をアーカイブ (`openspec archive-change` またはプロジェクト既存手順)
