## Context

### 現状の型表現と契約ずれ

kernel 層 `SupervisorStrategy` (`modules/actor-core/src/core/kernel/actor/supervision/base.rs:30`) は restart 上限を `max_restarts: u32` で保持し、`handle_failure` (base.rs:137-139) で以下のように扱う:

```rust
max_restarts:    u32,
...
let limit = if self.max_restarts == 0 { None } else { Some(self.max_restarts) };
let count = statistics.record_failure(now, self.within, limit);
if self.max_restarts > 0 && count as u32 > self.max_restarts {
  statistics.reset();
  SupervisorDirective::Stop
} else {
  SupervisorDirective::Restart
}
```

これは **`0 ⇒ 無制限 / > 0 ⇒ 最大 n 回`** という独自契約であり、Pekko の `FaultHandling.scala` の定義とは逆転している。

Pekko (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala` および `OneForOneStrategy` / `AllForOneStrategy` コンストラクタ):

| `maxNrOfRetries` | 意味 |
|-------------------|------|
| `-1` | 無制限 (unlimited) |
| `0` | retry なし (即 Stop) |
| `n > 0` | 最大 n 回 restart |

### typed 層の部分回避

`modules/actor-core/src/core/typed/restart_supervisor_strategy.rs:48-59` で以下のマジック値による回避が存在:

```rust
pub fn with_limit(self, max_restarts: i32, within: Duration) -> Self {
  let max_restarts = if max_restarts == -1 {
    0  // kernel の "unlimited" マーカー
  } else {
    assert!(max_restarts != 0, "max_restarts must be -1 or at least 1");
    match u32::try_from(max_restarts) { ... }
  };
  ...
}
```

この設計は:

1. `-1` マジック値に依存する非 Rust 的な API
2. Pekko で有効な `0 ⇒ 即 Stop` を panic で拒否しており、Pekko 契約を満たさない
3. kernel 層の反転した意味を typed 層で塗り替える形であり、kernel API を直接使う経路 (`MessageInvokerPipeline` 配線、将来の persistence / cluster supervision、テストサポート) には反転意味がそのまま露出

### Pekko `FaultHandling` の accumulator 契約

Pekko は restart 統計を以下のルールで管理する:

1. `withinTimeRange` が `Duration.Inf` (= Rust の `Duration::ZERO` と対応させているが、fraktor-rs では `0` をセンチネルで扱うか別途決定) の場合、失敗は累計され、window でリセットされない
2. `withinTimeRange > 0` の場合、window 外の失敗履歴は pruning される
3. decider が `Restart` **以外** を返した場合 (`Stop` / `Escalate` / `Resume`)、統計は **リセット** される
4. `maxNrOfRetries` が `-1` の場合、失敗カウンタは累計されるが上限判定は常に false

fraktor-rs の `RestartStatistics::record_failure` (`restart_statistics.rs:22-37`) は `max_history: Option<u32>` 引数で上限を受け取り、失敗履歴の pruning も行うが、`max_history` の意味 (履歴保持上限 ≠ Pekko の retry 上限) が kernel 層の反転判定と噛み合わず、contract が曖昧なまま放置されている。

### 制約

- **CLAUDE.md**: 後方互換は不要。破壊的変更を恐れず最適設計。
- **no_std core**: `modules/actor-core` は `alloc::*` のみ、`std::*` 禁止。`Option<u32>` / `NonZeroU32` / `Duration` は `core` で利用可能。
- **Pekko parity の原則**: 型名・シグネチャだけでなく **セマンティクス**まで Pekko と一致させる。
- **型表現の単純さ**: ambiguous-suffix / type-per-file dylint との整合。新規型を導入する場合は 1 型 1 ファイル。

## Goals / Non-Goals

**Goals:**

- kernel `SupervisorStrategy` の restart 上限型を **Pekko `maxNrOfRetries` の 3 値契約 (unlimited / no-retry / limited-n) と同型** の表現に置換する
- typed 層の `with_limit` から `i32 + -1 マジック値` を排除し、panic 経路を削減する。`0` を Pekko 契約どおり「retry なし」として受理可能にする
- `RestartStatistics::record_failure` の契約と `handle_failure` の decider 判定を Pekko accumulator と一致させる (`Restart 以外でリセット`、`within == 0` の扱いを明示化)
- 型シグネチャの変更に伴う全呼び出し箇所 (`backoff_supervisor_strategy`, `supervisor_strategy_config`, DSL `supervise`, テスト) を破壊的に更新
- `docs/gap-analysis/actor-gap-analysis.md` の SP-M1 を done 化

**Non-Goals:**

- AC-M3 (FailedFatally / isFailed ガード) の実装: 別 change
- AC-M5 (NotInfluenceReceiveTimeout マーカー): 別 change
- SP-H1 decider 粒度 (JVM Error → Escalate): 完了済み (`2026-04-21-2026-04-20-pekko-panic-guard`)
- Supervision strategy に関連する新機能の追加 (resumeWithDelay 等)
- BackoffSupervisor の再試行戦略の変更 (max_restarts の意味だけ揃える)
- `application.conf` / HOCON パーサの実装

## Decisions

### Decision 1: kernel 表現は `RestartLimit` 専用 enum を新設する

`max_restarts: u32` を以下の enum 型に置換する:

```rust
/// Maximum restart count policy (Pekko `maxNrOfRetries` equivalent).
///
/// - `Unlimited` mirrors Pekko `maxNrOfRetries = -1`.
/// - `WithinWindow(0)` mirrors Pekko `maxNrOfRetries = 0` (no retry, immediate stop).
/// - `WithinWindow(n)` mirrors Pekko `maxNrOfRetries = n` (up to n restarts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartLimit {
  Unlimited,
  WithinWindow(u32),
}
```

**選択肢と却下理由:**

- `Option<u32>` (`None = 無制限`): 最も軽量だが、`Some(0)` と `Some(n)` の意味差が呼び出し側で埋もれやすく、コードレビュー時に「0 は本当に即 Stop を意図しているか？」という注釈が毎回必要になる。enum の variant 名で意図を自己文書化する方が保守性が高い。
- `i32` + Pekko magic: 既に typed 側で使っているが、`-1` センチネルが Rust 的に非イディオマティック。却下。
- `NonZeroU32` + sentinel: `0` を意味的に区別する手段として魅力的だが、Pekko 契約では `0` が valid な値 (= no retry) であり、除外して別の型で表現するとかえって複雑化する。却下。

**利点:**

- variant 名 (`Unlimited` / `WithinWindow(0)`) が自己文書化され、Pekko 契約を直接対応付けられる
- パターンマッチが網羅的でコンパイラが分岐漏れを検出する
- typed 層 API の入力形を kernel に寄せられる (`with_unlimited()` / `within(n, duration)` のような流暢 API が自然)

### Decision 2: typed 層 API は Pekko 直訳の 2 メソッドに分解する

`restart_supervisor_strategy.rs` の `with_limit(i32, Duration)` を廃止し、以下のメソッドに差し替える:

```rust
impl RestartSupervisorStrategy {
  /// Allow up to `max_restarts` restarts within the given `within` window.
  /// Pekko equivalent: `OneForOneStrategy(maxNrOfRetries = max_restarts, withinTimeRange = within)`.
  pub fn with_limit(self, max_restarts: u32, within: Duration) -> Self { ... }

  /// Allow unlimited restarts within the given `within` window.
  /// Pekko equivalent: `OneForOneStrategy(maxNrOfRetries = -1, withinTimeRange = within)`.
  pub fn with_unlimited_restarts(self, within: Duration) -> Self { ... }
}
```

**選択肢と却下理由:**

- `with_limit(Option<u32>, Duration)`: 呼び出し側で `None` / `Some` を組み立てる必要があり、Pekko の `-1` / `0` / `n` とのマッピングが直観的でない。却下。
- 単一 `with_restart_limit(RestartLimit, Duration)`: kernel 型をそのまま typed 層へ露出する形。採用可だが、DSL 流暢性では 2 メソッドの方がコール箇所が自然 (`.with_unlimited_restarts(Duration::ZERO)` vs `.with_restart_limit(RestartLimit::Unlimited, Duration::ZERO)`)。保守的に 2 メソッド分解を選ぶ。

**`with_limit` の引数型は `u32`** に固定し、`0` は Pekko 契約どおり「retry なし、即 Stop」として受理する (panic しない)。

### Decision 3: `RestartStatistics` を Pekko one-shot window 状態に書き直し、上限判定 API を `request_restart_permission` に再設計

現行 `RestartStatistics` は `Vec<Duration>` に失敗タイムスタンプを蓄積し、`prune(window, now)` で window 外 (`now - window` 未満) を削除する **sliding window** アルゴリズム (`restart_statistics.rs:22-37,61-67`)。これは Pekko の `ChildRestartStats.retriesInWindowOkay` (`FaultHandling.scala:64-86`) の **one-shot window** (window 開始点を初回 restart で記録し、window 超過で全体を一度だけリセットして常に permit を返す) と異なる。`maxNrOfRetries` 反転修正だけでは Pekko parity を満たさず、内部 state も Pekko に揃える必要がある。

#### 新しい内部 state

```rust
pub struct RestartStatistics {
  restart_count: u32,
  window_start: Option<Duration>,  // None = window 未開始
}

impl RestartStatistics {
  pub const fn new() -> Self { Self { restart_count: 0, window_start: None } }

  pub fn reset(&mut self) { self.restart_count = 0; self.window_start = None; }

  pub const fn restart_count(&self) -> u32 { self.restart_count }
}
```

#### Pekko `requestRestartPermission` 相当 API

`record_failure(now, window, max_history) -> usize` を廃止し、Pekko の `ChildRestartStats.requestRestartPermission` (`FaultHandling.scala:56-62`) と同型の API に置換する:

```rust
/// Pekko `ChildRestartStats.requestRestartPermission` に対応 (FaultHandling.scala:56-62).
///
/// `limit` と `window` の組に応じて Pekko の 4 分岐を再現する:
/// - `(WithinWindow(0), _)` ⇒ false (counter 更新なし、即 Stop 契機)
/// - `(WithinWindow(n), ZERO)` ⇒ counter += 1; counter <= n (window なし)
/// - `(limit, window > 0)` ⇒ retries_in_window_okay(effective_retries, window, now)
/// - `(Unlimited, ZERO)` ⇒ true (counter 更新なし)
///
/// `limit = Unlimited` かつ `window > 0` の場合、Pekko は `retriesInWindowOkay(retries = 1, window)`
/// 経路を通るため window リセット挙動は走る。ただし `retriesDone <= retries = 1` で常に true を返す。
pub fn request_restart_permission(
  &mut self,
  now: Duration,
  limit: RestartLimit,
  window: Duration,
) -> bool {
  match (limit, window.is_zero()) {
    // Pekko `(Some(0), _) if retries < 1 => false`
    (RestartLimit::WithinWindow(0), _) => false,
    // Pekko `(None, _) => true` (window は None または treated as None sentinel)
    (RestartLimit::Unlimited, true) => true,
    // Pekko `(Some(n), None) => count += 1; count <= n` (saturating_add で u32 overflow 防止)
    (RestartLimit::WithinWindow(n), true) => {
      self.restart_count = self.restart_count.saturating_add(1);
      self.restart_count <= n
    }
    // Pekko `(None, Some(window)) => retriesInWindowOkay(retries = 1, window)`
    // ※ 注意: retries=1 固定のため window 内 2 回目以降は retriesDone > 1 で false
    //         を返す Pekko の quirk。`Unlimited + finite window` 明示指定時のみ顕在化
    (RestartLimit::Unlimited, false) => self.retries_in_window_okay(1, window, now),
    // Pekko `(Some(n), Some(window)) => retriesInWindowOkay(retries = n, window)`
    (RestartLimit::WithinWindow(n), false) => self.retries_in_window_okay(n, window, now),
  }
}

/// Pekko `retriesInWindowOkay` の直訳 (FaultHandling.scala:64-86).
fn retries_in_window_okay(&mut self, retries: u32, window: Duration, now: Duration) -> bool {
  let retries_done = self.restart_count.saturating_add(1);
  let window_start = match self.window_start {
    Some(ws) => ws,
    None => {
      self.window_start = Some(now);
      now
    }
  };
  let inside_window = now.saturating_sub(window_start) <= window;
  if inside_window {
    self.restart_count = retries_done;
    retries_done <= retries
  } else {
    // window expire: Pekko は carryover せず count=1 + window_start=now で再スタート
    self.restart_count = 1;
    self.window_start = Some(now);
    true
  }
}
```

#### `handle_failure` の orchestration

上限判定は `request_restart_permission` 側に移動し、`handle_failure` は directive 別の accumulator 更新だけを残す:

```rust
match self.decide(error) {
  SupervisorDirective::Restart => {
    if statistics.request_restart_permission(now, self.max_restarts, self.within) {
      SupervisorDirective::Restart
    } else {
      statistics.reset();
      SupervisorDirective::Stop
    }
  }
  SupervisorDirective::Stop => { statistics.reset(); SupervisorDirective::Stop }
  SupervisorDirective::Escalate => { statistics.reset(); SupervisorDirective::Escalate }
  SupervisorDirective::Resume => SupervisorDirective::Resume,  // Pekko: Resume は childStats に触れない
}
```

**選択肢と却下理由:**

- 現状維持 (sliding window + `record_failure` を 2 引数化): Pekko `requestRestartPermission` が one-shot window + pre-check (`Some(0)` で counter 非更新) を前提とするため、sliding window のままだと Scenario が Pekko と一致しない (count の時系列挙動が異なる)。却下。
- `RestartStatistics` 内で `RestartLimit` を保持: `new()` に policy 引数が必要になり、strategy 側の独立性が崩れる。Pekko も `ChildRestartStats` は `retriesWindow: (Option[Int], Option[Int])` を呼び出し時引数で受ける設計であり、引数渡しが Pekko 設計に忠実。却下。
- `Vec<Duration>` を残しつつ one-shot window をシミュレート: 履歴保持のメモリコストが不要であり、Pekko 実装と乖離した余計な state を抱える。却下。

**CQS 原則との関係 (`.agents/rules/rust/cqs-principle.md`)**:

`request_restart_permission(&mut self, ...) -> bool` は状態更新 (`restart_count` / `window_start` 書き換え) と bool 返却を同時に行うため、CQS 原則 (「状態を変更するメソッドは戻り値を返さない」) に違反する。ただし以下の理由で **人間の許可を前提に CQS 違反を許容** する:

- Pekko 参照実装の `ChildRestartStats.requestRestartPermission` (`FaultHandling.scala:55-62`) 自体が「FIXME How about making ChildRestartStats immutable ...」と Pekko 開発者に認識されつつ同じ設計を採用しており、問い合わせと更新の分離は TOCTOU を招くため採用されていない
- 仮に「permit 判定」と「permit 適用 (= counter / window_start 更新)」を 2 メソッドに分離すると、呼び出し側で `if can_permit(...) { apply_permit(...); Restart } else { reset(); Stop }` の並びが必要になり、`can_permit` 評価時の状態と `apply_permit` 実行時の状態が乖離する TOCTOU が発生する (ロック境界では閉じていても、ロック外で誤運用が起きやすい)
- CQS 違反の影響範囲は supervision 内部の `handle_failure` orchestration のみに限定され、公開 API 境界から外へは漏れない (呼び出し側は `handle_failure` 1 呼び出しだけを扱う)
- 現状 fraktor-rs の `Vec::pop` / `Iterator::next` と同じく「ロジック上分離不可な CQS 違反」として例外扱いする (`cqs-principle.md` 「許容される違反」節)

この例外は本 change レビュー時に人間レビュアが明示的に承認する前提とし、rustdoc でも CQS 違反を意図的な設計判断として明記する。

### Decision 4: `within == Duration::ZERO` を「window なし」のセンチネルとして採用する (typed 層 Pekko の既定と一致)

**採用の根拠 (typed 層 Pekko と同形)**:

typed 層 Pekko `SupervisorStrategy.scala:44-45` では `Restart` strategy の default が以下の通り:

```scala
Restart(maxRestarts = -1, withinTimeRange = Duration.Zero)
```

この `Duration.Zero` は「`maxRestarts = -1` (unlimited) と組み合わせて window なしを示すデフォルト」として使われており、typed 層の意味論では **`Duration.Zero = window なし**」である。fraktor-rs の typed 層 (`RestartSupervisorStrategy` / `BackoffSupervisorStrategy`) は Pekko typed 層の API 契約に対応づけるため、`Duration::ZERO` を typed Pekko と同じく「window なし」のセンチネルとして採用する。

**classic 層 Pekko との差異 (参考・本 change の直接の対象外)**:

classic 層 Pekko の `SupervisorStrategy.withinTimeRangeOption` (`FaultHandling.scala:300-304`) は以下:

```scala
private[pekko] def withinTimeRangeOption(withinTimeRange: Duration): Option[Duration] =
  if (withinTimeRange.isFinite && withinTimeRange > Duration.Zero) Some(withinTimeRange) else None
```

classic 層では `Duration.Zero` (0ms) は `.isFinite == true` だが `> Duration.Zero == false` のため `None` 扱い、つまり `Duration.Zero` と `Duration.Inf` の双方が「window なし」として集約される。結果として classic / typed の双方とも `Duration.Zero` を「window なし」として扱う点で一致しており、fraktor-rs の `Duration::ZERO = window なし` センチネルは **両層と矛盾しない**。

**注意書き (rustdoc MUST)**:

- `SupervisorStrategy::new` / `BackoffSupervisorStrategy::with_within` / `RestartSupervisorStrategy::with_limit(n, within)` / `with_unlimited_restarts(within)` / `RestartLimit` / `RestartStatistics::request_restart_permission` の rustdoc で **「`Duration::ZERO` は typed Pekko `Duration.Zero` / classic Pekko `withinTimeRangeOption` が `None` を返すケースに対応し、window なし (無期限累計) を示す」** と明記する
- 将来「明示的な 0ms window」の表現が必要になった場合 (例: バースト制御) は `Option<Duration>` による明示表現への置換を follow-up change で検討する (本 change では対象外)

### Decision 5: `SupervisorDirective::Resume` は `RestartStatistics` を一切変更しない

Pekko `FaultHandling.scala` の `handleFailure` 相当処理を読むと、`Resume` directive では `ChildRestartStats.requestRestartPermission` を呼ばず `childStats` も更新しない。現状 fraktor-rs の `base.rs:154` も `Resume` arm で `statistics.reset()` を呼ばず素通しになっているが、**仕様として Pekko と一致する意図を明文化** する。

```rust
SupervisorDirective::Resume => SupervisorDirective::Resume,
// Pekko `FaultHandling` 相当: Resume は childStats を一切触らない。
// statistics.reset() / statistics.request_restart_permission() は呼ばない。
```

これにより、`Resume` 発火後に再度 `Restart` が起きた場合でも、それ以前の failure history が保持され、`WithinWindow(n)` / `retries_in_window_okay` が Pekko と同じ timeline で判定される。

## Risks / Trade-offs

- **[Risk] 呼び出し箇所の全置換漏れ** → Mitigation: kernel 側 `max_restarts: u32` を削除したコンパイルエラーを起点に、すべての `.max_restarts()` / `.with_max_restarts()` / `with_limit(i32, ...)` コールをコンパイラ経路で強制検出。`rtk cargo check --workspace` を task の早期ゲートに置く
- **[Risk] 既存テストの Pekko 逆転依存** → Mitigation: `assert_eq!(strategy.max_restarts(), 0)` で「0 = unlimited」を暗黙に期待するテストを grep で洗い出し、新表現 (`RestartLimit::Unlimited`) へ書き換え。テスト側でも Pekko 契約に従った assert に統一
- **[Risk] BackoffSupervisorStrategy の retry 上限が別系統** → Mitigation: `backoff_supervisor_strategy.rs` の `max_restarts` フィールドも同時に `RestartLimit` に統一し、Pekko の `BackoffSupervisor` と意味を揃える。`BackoffSupervisorStrategyConfig` の builder も同じ API に合わせる
- **[Trade-off] enum 型導入によるサイズ増** → `RestartLimit` は `enum { Unlimited, WithinWindow(u32) }` で 8 バイト程度 (tag + u32)。`u32` 単体の 4 バイトから 2 倍になるが、`SupervisorStrategy` は configure 時にのみ構築される低頻度 struct であり、実行時 hotpath ではない。問題なし
- **[Trade-off] typed 層 API が 2 メソッドに分かれる** → `with_limit` と `with_unlimited_restarts` の 2 つを用意。1 メソッドでは Pekko の `-1` / `0` / `n` の 3 契約が直観的に表現できないため、DSL 流暢性を優先して分解を許容

## Migration Plan

1. `RestartLimit` enum 新設 (`restart_limit.rs`) と `SupervisorStrategy::max_restarts` 型置換
2. `RestartStatistics::record_failure` から `max_history` 引数を削除し、`handle_failure` 側に上限判定を移動
3. `BackoffSupervisorStrategy` / `BackoffSupervisorStrategyConfig` / `supervisor_strategy_config.rs` の `u32` 参照を `RestartLimit` に差し替え
4. typed 層 `with_limit` を `(u32, Duration)` 版に差し替え、`with_unlimited_restarts(Duration)` を追加。`should_panic` テストを削除し、`0 ⇒ Stop` テストに書き換え
5. 全 `tests.rs` を新契約に書き換え、Pekko 契約と一致する Scenario を追加
6. `docs/gap-analysis/actor-gap-analysis.md` SP-M1 行を done 化、残存 medium 数を 13 → 12 に更新
7. `./scripts/ci-check.sh ai all` を final ゲートとして実行

ロールバック: 破壊的変更のため roll-forward only。異常があれば個別 follow-up PR で修正する。

## Open Questions

- **`RestartLimit::WithinWindow(0)` の Display / Debug 表現**: 「0 回 retry = 即 Stop」をユーザが誤読しないよう `Debug` 実装でコメントを残すか？ → 実装時に判断し、rustdoc で Pekko `maxNrOfRetries = 0` とのマッピングを明示する方針で暫定合意
- **`with_limit(0, within)` は panic すべきか？** → しない。Pekko 契約どおり「即 Stop」として受理する。これは「retry なし restart strategy」であり、意味的には valid な設定
- **`BackoffSupervisorStrategy` の max_restarts は `RestartLimit` を持つべきか、別型か？** → 同じ `RestartLimit` を流用する。`BackoffSupervisor` も Pekko では `maxNrOfRetries` を同じ契約で扱うため、型を共有することで整合が取れる
