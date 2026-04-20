## プロジェクト原則（全 change 共通）

本 change は以下 4 原則に従って設計される:

1. **Pekko 互換仕様の実現 + Rust らしい設計**: Pekko dispatcher の `try/catch` による panic-to-ActorFailure 変換を、Rust の `std::panic::catch_unwind` + `ActorError::Escalate` 変換に翻訳する。trait は dyn-compatible + `Self: Sized` default method で「持ち運び可能な抽象」と「具象型の書き味」を両立
2. **手間が掛かっても本質的な設計を選ぶ**: `MessageInvokerPipeline` にジェネリック `<G: InvokeGuard>` を伝播させて全階層を汚染する選択肢を避け、`ArcShared<Box<dyn InvokeGuard>>` + `InvokeGuardFactory` による config 経由注入という dyn-based 設計を採用する
3. **フォールバックや後方互換性を保つコードを書かない**: `MessageInvokerPipeline::new()` を廃止し、`new_with_guard` のみを公開する破壊的変更。既存 caller は全て新 API に更新し、暫定的な fallback constructor を残さない
4. **no_std core + std adaptor 分離**: `InvokeGuard` trait と `NoopInvokeGuard` は `modules/actor-core`（no_std 維持、`std::panic` 禁止）、`PanicInvokeGuard` は `modules/actor-adaptor-std`。`cfg-std-forbid` lint で機械的に違反検出する

## Why

Pekko dispatcher は actor の `receiveMessage` 呼び出しを `try/catch` で包み、`Throwable` を `ActorFailure` として supervisor にエスカレーションする。fraktor-rs は現状 `std::panic::catch_unwind` を呼ぶ箇所を持たず、panic が発生すると worker thread が終了し supervisor 経路をバイパスしてしまう（gap-analysis SP-H1.5）。

本 change は std adaptor 層に `PanicInvokeGuard` を新設して `actor.receive()` を包囲し、panic を `ActorError::Escalate(ActorErrorReason::new(panic_msg))` に変換する。no_std core は `NoopInvokeGuard` の素通し実装を提供し、panic 変換責務は std adaptor 層に限定する。

ブランチには既に SP-H1.5 テスト 4 件 (`modules/actor-adaptor-std/tests/sp_h1_5_panic_guard.rs`) が書かれており、`fraktor_actor_core_rs::core::kernel::actor::invoke_guard::{InvokeGuard, NoopInvokeGuard}` / `fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard` を期待するが production code に未存在。本 change でこれらを実装して passing にする。

## What Changes

### 1. `InvokeGuard` trait（dyn-compatible + `Self: Sized` default method）

- `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` を新規作成
  - **dyn-compatible** な `wrap_receive` を定義し、`ArcShared<Box<dyn InvokeGuard>>` で持ち運べる形にする:

  ```rust
  pub trait InvokeGuard: Send + Sync {
      /// dyn-compatible 本体: kernel の `MessageInvokerPipeline` から呼ばれる経路
      fn wrap_receive(
          &self,
          call: &mut dyn FnMut() -> Result<(), ActorError>,
      ) -> Result<(), ActorError>;

      /// `guard.wrap(|| ...)` 書き味を提供する default method。
      /// `Self: Sized` により trait の dyn-compatibility は維持される
      /// （`dyn InvokeGuard` 経由では呼べず、具象型の method resolution 経由でのみ呼べる）。
      fn wrap<F>(&self, f: F) -> Result<(), ActorError>
      where
          F: FnOnce() -> Result<(), ActorError>,
          Self: Sized,
      {
          let mut opt = Some(f);
          self.wrap_receive(&mut || opt.take().expect("wrap_receive called closure more than once")())
      }
  }
  ```

  - `pub struct NoopInvokeGuard;` (no_std default): `wrap_receive` は `call()` 素通し
  - この設計により既存テスト (`sp_h1_5_panic_guard.rs`) の `use ...::{InvokeGuard, NoopInvokeGuard}; guard.wrap(|| ...)` が import 変更なしで動作する
- `modules/actor-core/src/core/kernel/actor.rs` に `pub mod invoke_guard;` 追加

### 2. `ActorSystemConfig` への guard factory 設定面追加

現状 `ActorSystem::create_with_config*` は core 側 (`modules/actor-core/src/core/kernel/system/base.rs:129`) にあり、`ActorCell` 構築 (`actor_cell.rs:181`) は `MessageInvokerPipeline::new()` を固定で呼ぶため、**std adaptor builder から直接 guard を注入する経路が存在しない**。これを解消する:

- `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs` に `invoke_guard_factory: Option<ArcShared<Box<dyn InvokeGuardFactory>>>` フィールドを追加
- `pub trait InvokeGuardFactory: Send + Sync { fn build(&self) -> ArcShared<Box<dyn InvokeGuard>>; }` を `invoke_guard.rs` に定義（または新規 `invoke_guard_factory.rs`、type-per-file 判定による）
- `ActorSystemConfig::with_invoke_guard_factory(factory: ArcShared<Box<dyn InvokeGuardFactory>>) -> Self` setter を追加
- 未設定時は `ArcShared::new(Box::new(NoopInvokeGuardFactory))` が default（`ArcShared::new(Box::new(NoopInvokeGuard))` を返すだけ）
- `SystemState` が `ActorSystemConfig::take_invoke_guard_factory()` 経由で factory を保持
- `SystemState` から全 `ActorCell::new(...)` 経路へ `ArcShared<Box<dyn InvokeGuard>>` を配布

### 3. `MessageInvokerPipeline` / `ActorCell` の改修

- `pipeline.rs` の `invoke_user` 内 `actor.receive(ctx, view)` を `self.guard.wrap_receive(&mut || actor.receive(ctx, view))` で包む
- `MessageInvokerPipeline::new()` を `MessageInvokerPipeline::new_with_guard(guard: ArcShared<Box<dyn InvokeGuard>>)` に差し替え（既存 `new()` を削除し、すべての caller を新シグネチャに更新）
- `ActorCell::new` 経路で `system.invoke_guard()` から factory を通じて `ArcShared<Box<dyn InvokeGuard>>` を取得し pipeline に渡す
- **generic parameter は伝播させない**（`dyn InvokeGuard` で解決するため、既存の `Spawn<Sp, Si, Tk>` 階層に影響なし）

### 4. std adaptor 側の factory 実装

- `modules/actor-adaptor-std/src/std/actor/` 新規モジュール
  - `std/actor.rs` + `std/actor/panic_invoke_guard.rs`
  - `pub struct PanicInvokeGuard;` と `impl InvokeGuard for PanicInvokeGuard`
    - `wrap_receive` 内で `std::panic::catch_unwind(AssertUnwindSafe(|| call()))`
    - panic 捕捉時 `Err(ActorError::Escalate(ActorErrorReason::new(format!("panic: {msg}"))))`
    - panic 以外（`Ok` / `Err(_)`）は素通し
  - `pub struct PanicInvokeGuardFactory;` と `impl InvokeGuardFactory for PanicInvokeGuardFactory { fn build(&self) -> ArcShared<Box<dyn InvokeGuard>> { ArcShared::new(Box::new(PanicInvokeGuard)) } }`
  - `pub fn install_panic_invoke_guard(config: ActorSystemConfig) -> ActorSystemConfig { config.with_invoke_guard_factory(ArcShared::new(Box::new(PanicInvokeGuardFactory))) }` 等の helper を公開
- `modules/actor-adaptor-std/src/std.rs` に `pub mod actor;` 追加

## Capabilities

### New Capabilities
- `pekko-panic-guard`: actor の `receive` で発生した panic を std adaptor 層で `ActorError::Escalate` に変換する guard 機構（`InvokeGuard` trait + `NoopInvokeGuard` (no_std) + `PanicInvokeGuard` (std adaptor)）

注: `actor-std-adapter-surface` は「std 公開面は adapter と std 固有 helper のみ」を定めており、`PanicInvokeGuard` は std 固有 helper に該当するため既存制約を破らない。`actor-package-structure` は `kernel` / `typed` の最上位境界を定めるのみで、`kernel/actor` 配下のサブモジュール追加は既存 Requirement を破らない。よって両 capability の MODIFIED は不要。

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` (新規)
  - `modules/actor-core/src/core/kernel/actor.rs` (mod 追加 + re-export)
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs` (`invoke_guard_factory` フィールドと setter 追加)
  - `modules/actor-core/src/core/kernel/system/state/system_state.rs` (`invoke_guard_factory` 保持 + getter)
  - `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/pipeline.rs` (`new()` 廃止 → `new_with_guard(ArcShared<Box<dyn InvokeGuard>>)` 置換、`wrap_receive` で `receive` 包囲)
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs` (`ActorCell::create` で factory から guard を取得して pipeline に渡す)
  - `modules/actor-adaptor-std/src/std.rs` (`pub mod actor;` 追加)
  - `modules/actor-adaptor-std/src/std/actor.rs` + `std/actor/panic_invoke_guard.rs` (新規)
  - テスト: `modules/actor-adaptor-std/tests/sp_h1_5_panic_guard.rs` (既存 4 件を passing に)
- 影響内容:
  - kernel 層 default は `NoopInvokeGuardFactory` で panic semantics は現状維持
  - std adaptor 利用者が `install_panic_invoke_guard(config)` を呼ぶと `PanicInvokeGuardFactory` が `ActorSystemConfig` にセットされ、全 `ActorCell` は factory `build()` から得た state-less な `PanicInvokeGuard` 由来の `ArcShared<Box<dyn InvokeGuard>>` を pipeline に注入される（各 cell の guard instance は別だが state-less のため挙動は完全に同一）
  - `ArcShared<Box<dyn InvokeGuard>>` 経由で持ち運ぶため、`MessageInvokerPipeline` 以下に generic parameter を伝播させない（既存の `Spawn<Sp, Si, Tk>` 階層への影響なし）
  - `MessageInvokerPipeline::new()` が消えるため、`ActorCell::new` や test helper 経由で pipeline を構築する箇所は `new_with_guard(guard)` に置換が必要
- 非目標:
  - lifecycle hooks (`pre_start` / `post_stop` / `pre_restart` / `post_restart`) への panic guard（Phase A3 で別途評価）
  - `ctx` の可変借用中 panic の二次影響評価（Phase A3）
  - `MessageInvokerPipeline` への generic parameter 伝播（`ArcShared<Box<dyn InvokeGuard>>` + dyn-compatible trait で解決するため不要）

## 依存関係

- **`2026-04-20-pekko-restart-completion` を先に merge する必要がある**（同ブランチは kernel ビルドエラー状態のため、本 change の既存テスト `tests/sp_h1_5_panic_guard.rs` も workspace ビルド不可）
- `2026-04-20-pekko-eventstream-subchannel` とは独立（モジュール境界が分かれており並列実装可）
