# remote-core-failure-detector Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: PhiAccrualFailureDetector 型

`fraktor_remote_core_rs::core::failure_detector::PhiAccrualFailureDetector` 型が定義され、Phi Accrual algorithm に基づく failure detector を実装する SHALL。Pekko `PhiAccrualFailureDetector` (Scala, 295行) に対応する。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/failure_detector/phi_accrual.rs` または同等のファイルを読む
- **THEN** `pub struct PhiAccrualFailureDetector` が定義されている

#### Scenario: コンストラクタ

- **WHEN** `PhiAccrualFailureDetector::new` の定義を読む
- **THEN** `fn new(threshold: f64, max_sample_size: usize, min_std_deviation: u64, acceptable_heartbeat_pause: u64, first_heartbeat_estimate: u64) -> Self` または同等のシグネチャが宣言されている

### Requirement: 純関数としての時刻入力 (monotonic millis)

`PhiAccrualFailureDetector` のすべての操作 (heartbeat 記録、phi 計算、is_available 判定) は時刻を **monotonic millis** として引数で受け取り、`Instant::now()` や `SystemTime::now()` を内部で呼ばない SHALL。wall clock 由来の時刻 (`SystemTime`) を渡すことは禁止である — wall clock のジャンプで failure 判定が誤動作するため。adapter 側で `std::time::Instant` や `tokio::time::Instant` の差分を計算して渡すことが想定される。

#### Scenario: heartbeat メソッド

- **WHEN** `PhiAccrualFailureDetector::heartbeat` の定義を読む
- **THEN** `fn heartbeat(&mut self, now_ms: u64 /* monotonic millis */)` または同等のシグネチャが宣言されており、doc comment で monotonic millis であることが明示されている

#### Scenario: phi メソッド

- **WHEN** `PhiAccrualFailureDetector::phi` の定義を読む
- **THEN** `fn phi(&self, now_ms: u64 /* monotonic millis */) -> f64` または同等のシグネチャが宣言されている (`&self` の query、CQS 準拠)

#### Scenario: is_available メソッド

- **WHEN** `PhiAccrualFailureDetector::is_available` の定義を読む
- **THEN** `fn is_available(&self, now_ms: u64 /* monotonic millis */) -> bool` または同等のシグネチャが宣言されている

#### Scenario: doc comment の monotonic 明示

- **WHEN** `PhiAccrualFailureDetector` の公開メソッドの rustdoc を読む
- **THEN** `now` パラメータが **monotonic** (wall clock ではない) であることが明示されており、wall clock を渡した場合の挙動 (「単調性が破れると誤検知が起きる」) が警告されている

#### Scenario: Instant 直接呼び出しの不在

- **WHEN** `modules/remote-core/src/failure_detector/` 配下のすべての `.rs` ファイルを検査する
- **THEN** `Instant::now()`・`SystemTime::now()`・`std::time::` の参照が存在しない

### Requirement: heartbeat history の保持

`PhiAccrualFailureDetector` は内部に heartbeat 間隔の履歴 (`HeartbeatHistory` 等) を保持し、`max_sample_size` を上限とする ring buffer として動作する SHALL。

#### Scenario: HeartbeatHistory の存在

- **WHEN** `modules/remote-core/src/failure_detector/` 配下を検査する
- **THEN** `pub struct HeartbeatHistory` または同等の履歴保持型が存在する

#### Scenario: max_sample_size を超える履歴の切り詰め

- **WHEN** `max_sample_size = 100` で `PhiAccrualFailureDetector` を作成し、200 回 `heartbeat(now)` を異なる時刻で呼ぶ
- **THEN** 内部の履歴サイズは100以下に保たれる

### Requirement: 標準偏差の最小値保証

`PhiAccrualFailureDetector` の phi 計算は、heartbeat 間隔の標準偏差が `min_std_deviation` 未満の場合は `min_std_deviation` を使用する SHALL。これにより異常に小さい標準偏差による phi 値の発散を防ぐ。

#### Scenario: min_std_deviation の適用

- **WHEN** すべての heartbeat 間隔が完全に一定 (標準偏差0) の状態で `phi(now)` を呼ぶ
- **THEN** 計算結果に NaN や Infinity が含まれず、有限の値が返る

### Requirement: no_std + alloc での動作

`PhiAccrualFailureDetector` は `std` を使わず、`alloc::collections::VecDeque` または同等のコレクションのみで動作する SHALL。

#### Scenario: alloc のみへの依存

- **WHEN** `modules/remote-core/src/failure_detector/` 配下のすべての import を検査する
- **THEN** `use std::` を含む行が存在せず、コレクションは `alloc::` または `core::` から取得されている

