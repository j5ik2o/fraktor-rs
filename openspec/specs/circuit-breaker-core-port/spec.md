# circuit-breaker-core-port Specification

## Purpose
TBD - created by archiving change actor-core-std-separation-improvement. Update Purpose after archive.
## Requirements
### Requirement: Clock Port trait を core/pattern/ に定義する

`core::pattern::clock` モジュールに `Clock` trait を定義する。この trait は時刻取得と経過時間の計算を抽象化し、no_std 環境でもタイムアウトベースのロジックを記述可能にする。

trait は以下のシグネチャを持つ:
- `type Instant: Copy + Ord + Send + Sync` — 不透明な時点型
- `fn now(&self) -> Self::Instant` — 現在時刻を返す
- `fn elapsed_since(&self, earlier: Self::Instant) -> Duration` — 経過時間を返す

#### Scenario: Clock trait が no_std 環境でコンパイルできる
- **WHEN** `#![no_std]` 環境で `core::pattern::Clock` を使用する
- **THEN** コンパイルが成功する（std クレートへの依存がない）

#### Scenario: Clock trait の Instant 型で時間比較ができる
- **WHEN** `Clock::now()` で取得した2つの `Instant` 値を比較する
- **THEN** `Ord` trait により順序比較が可能である

### Requirement: StdClock を std/pattern/ に実装する

`std::pattern::StdClock` 構造体を定義し、`std::time::Instant` を使って `Clock` trait を実装する。

#### Scenario: StdClock が std::time::Instant ベースで動作する
- **WHEN** `StdClock::new()` で生成し `now()` を呼び出す
- **THEN** `std::time::Instant::now()` に基づく時刻が返される

#### Scenario: StdClock の elapsed_since が正しい Duration を返す
- **WHEN** `StdClock::now()` で `t1` を取得し、一定時間後に `elapsed_since(t1)` を呼び出す
- **THEN** 経過した `Duration` が返される

### Requirement: CircuitBreaker を Clock trait でジェネリック化して core/ に移設する

`CircuitBreaker<C: Clock>` として型パラメータ化し、`core::pattern::circuit_breaker` モジュールに配置する。既存の `Box<dyn Fn() -> Instant>` による clock injection を `Clock` trait に置き換える。

以下のファイルを `std/pattern/` から `core/pattern/` に移設する:
- `circuit_breaker.rs` → `core/pattern/circuit_breaker.rs`
- `circuit_breaker_state.rs` → `core/pattern/circuit_breaker_state.rs`
- `circuit_breaker_call_error.rs` → `core/pattern/circuit_breaker_call_error.rs`
- `circuit_breaker_open_error.rs` → `core/pattern/circuit_breaker_open_error.rs`
- `circuit_breaker_shared.rs` → `core/pattern/circuit_breaker_shared.rs`

#### Scenario: CircuitBreaker が no_std 環境でコンパイルできる
- **WHEN** `#![no_std]` 環境で `core::pattern::CircuitBreaker<C>` を使用する（`C: Clock` を提供）
- **THEN** コンパイルが成功する

#### Scenario: CircuitBreaker の状態遷移ロジックが維持される
- **WHEN** `max_failures` 回の失敗を記録する
- **THEN** 状態が `Closed` → `Open` に遷移する

#### Scenario: CircuitBreaker の Open → HalfOpen 遷移がタイムアウトで発生する
- **WHEN** `Open` 状態で `reset_timeout` 以上の時間が経過する
- **THEN** 状態が `HalfOpen` に遷移する

#### Scenario: CircuitBreakerShared もジェネリック化される
- **WHEN** `CircuitBreakerShared<C: Clock>` を使用する
- **THEN** `call()` メソッドで CircuitBreaker の保護下で非同期呼び出しが実行できる

### Requirement: std/pattern/ で型エイリアスを提供する

`std::pattern` モジュールで以下の型エイリアスを公開し、std ユーザーが型パラメータを意識せずに使えるようにする:
- `type CircuitBreaker = core::pattern::CircuitBreaker<StdClock>`
- `type CircuitBreakerShared = core::pattern::CircuitBreakerShared<StdClock>`

ファクトリメソッドも std 側に残し、`StdClock` を自動注入する。

#### Scenario: std ユーザーが型パラメータなしで CircuitBreaker を使用できる
- **WHEN** `use crate::std::pattern::CircuitBreaker` で import する
- **THEN** `CircuitBreaker::new(max_failures, reset_timeout)` が型パラメータなしで呼び出せる

#### Scenario: std の re-export が既存テストと互換である
- **WHEN** 既存の `std::pattern::CircuitBreaker` を使用するテストをコンパイルする
- **THEN** 型エイリアス経由で変更なくコンパイルが通る（ファクトリメソッドのシグネチャが維持される）

