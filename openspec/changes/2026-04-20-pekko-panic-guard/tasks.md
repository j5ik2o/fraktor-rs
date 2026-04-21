## 0. 前提確認

- [x] 0.1 既存テスト `modules/actor-adaptor-std/tests/sp_h1_5_panic_guard.rs` の `sp_h1_5_t1..t4` 4 件が存在することを確認する
- [x] 0.2 テストが期待する import パスを確認する
  - `fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard`
  - `fraktor_actor_core_rs::core::kernel::actor::invoke_guard::{InvokeGuard, NoopInvokeGuard}`
- [x] 0.3 `cfg-std-forbid` lint の対象を確認（core には `std::panic` を持ち込まないこと）
- [x] 0.4 現状 `ActorSystem::create_with_config*` は core 側 (`modules/actor-core/src/core/kernel/system/base.rs:129`)、`ActorCell::new` は `MessageInvokerPipeline::new()` 固定 (`actor_cell.rs:181`)、`modules/actor-adaptor-std/src/std.rs` に ActorSystem 用 builder surface なし、を確認する。この前提から「config 経由の guard factory 注入」方針をとる

## 1. kernel 側 `InvokeGuard` trait と `NoopInvokeGuard`

- [x] 1.1 `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` を新規作成し、**dyn-compatible + `Self: Sized` default method** 形の trait を定義する
  - `pub trait InvokeGuard: Send + Sync` を定義
    - `fn wrap_receive(&self, call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError>;` を要実装メソッドとする
    - `fn wrap<F: FnOnce() -> Result<(), ActorError>>(&self, f: F) -> Result<(), ActorError> where Self: Sized` を default method として追加（内部で `wrap_receive` を呼ぶ）
  - `Self: Sized` により `dyn InvokeGuard` 経由では `wrap` が呼べず dyn-compatibility は維持される。具象型 (`NoopInvokeGuard` / `PanicInvokeGuard`) の method resolution では `wrap` が見える
  - **この設計により既存テスト `sp_h1_5_panic_guard.rs:14-18` の `use ...::{InvokeGuard, NoopInvokeGuard}` import で `guard.wrap(|| ...)` が動作する。import 変更不要**
  - rustdoc は英語。「`PanicInvokeGuard` は std adaptor 側」「`NoopInvokeGuard` は no_std default」「`ArcShared<Box<dyn InvokeGuard>>` で持ち運ぶが `wrap` は具象型のみ」を記述
- [x] 1.2 `pub struct NoopInvokeGuard;` を同ファイルまたはサブモジュール (`invoke_guard/noop_invoke_guard.rs`) に追加（type-per-file lint 判定に従う）
  - `impl NoopInvokeGuard { pub const fn new() -> Self { Self } }`
  - `impl InvokeGuard for NoopInvokeGuard { fn wrap_receive(&self, call) -> ... { call() } }`（panic 非捕捉）
- [x] 1.3 `modules/actor-core/src/core/kernel/actor.rs` に `pub mod invoke_guard;` を追加、`pub use invoke_guard::{InvokeGuard, NoopInvokeGuard};`（`InvokeGuardExt` は不要）
- [x] 1.4 `rtk cargo check -p fraktor-actor-core-rs` がビルドされることを確認
- [x] 1.5 `./scripts/ci-check.sh ai dylint` が exit 0

## 2. `InvokeGuardFactory` trait と `ActorSystemConfig` への注入経路

- [x] 2.1 `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` (or 新規 `invoke_guard/invoke_guard_factory.rs`) に factory trait を追加
  - `pub trait InvokeGuardFactory: Send + Sync { fn build(&self) -> ArcShared<Box<dyn InvokeGuard>>; }`
  - `pub struct NoopInvokeGuardFactory;` + `impl InvokeGuardFactory for NoopInvokeGuardFactory { fn build(&self) -> ArcShared<Box<dyn InvokeGuard>> { ArcShared::new(Box::new(NoopInvokeGuard)) } }`
- [x] 2.2 `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs` の `ActorSystemConfig` に `invoke_guard_factory: Option<ArcShared<Box<dyn InvokeGuardFactory>>>` フィールドと `with_invoke_guard_factory(factory: ArcShared<Box<dyn InvokeGuardFactory>>) -> Self` setter を追加
  - 未設定時の default getter は `ArcShared::new(Box::new(NoopInvokeGuardFactory))`
- [x] 2.3 `SystemState::build_from_owned_config` 経路で factory を `SystemState` に保持し、`fn invoke_guard_factory(&self) -> &ArcShared<Box<dyn InvokeGuardFactory>>` getter を追加
- [x] 2.4 `ActorSystemConfig::take_invoke_guard_factory()` を追加する
  - 判定基準: `actor_system_config.rs` の既存 `take_*` 群（`take_dispatcher` / `take_mailbox_factory` 等）と同じ builder 消費パターンを採用し、`SystemState::build_from_owned_config` 内で config を move-consume する際に他のフィールドと統一的に取得できるようにする
  - 例外: `ActorSystemConfig` が他フィールドを `take_*` でなく `clone` / `&` 参照で取り出している場合のみ本タスクをスキップする。`actor_system_config.rs` 上で `take_*` 形式が存在する場合は追加必須
- [x] 2.5 `./scripts/ci-check.sh ai dylint` が exit 0

## 3. `MessageInvokerPipeline` と `ActorCell` の改修

- [x] 3.1 `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/pipeline.rs` を改修
  - `MessageInvokerPipeline` 構造体に `guard: ArcShared<Box<dyn InvokeGuard>>` フィールド追加
  - 既存 `MessageInvokerPipeline::new()` を `MessageInvokerPipeline::new_with_guard(guard: ArcShared<Box<dyn InvokeGuard>>)` に置換（下位互換維持しない、破壊的変更）
  - `invoke_user` 内 `actor.receive(ctx, view)` 呼び出しを `self.guard.wrap_receive(&mut || actor.receive(ctx, view))` に置換
- [x] 3.2 `modules/actor-core/src/core/kernel/actor/actor_cell.rs:181` の `pipeline: MessageInvokerPipeline::new()` を `pipeline: MessageInvokerPipeline::new_with_guard(system.invoke_guard_factory().build())` に変更
- [x] 3.3 `MessageInvokerPipeline::new()` の呼び出し箇所を workspace 全体で grep して、test helper / 他 builder も `new_with_guard` に更新（既存 pipeline-less テストがあれば `ArcShared::new(Box::new(NoopInvokeGuard))` を直接渡す）
- [x] 3.4 `rtk cargo test -p fraktor-actor-core-rs` で kernel 層 default (`NoopInvokeGuardFactory`) の挙動が従来と完全一致することを確認（1792+ 件 passing）
- [x] 3.5 `./scripts/ci-check.sh ai dylint` が exit 0

## 4. std adaptor 側 `PanicInvokeGuard` + factory + helper

- [x] 4.1 `modules/actor-adaptor-std/src/std/actor.rs` (親モジュールファイル) を新規作成
  - `pub mod panic_invoke_guard;`
  - `pub use panic_invoke_guard::{PanicInvokeGuard, PanicInvokeGuardFactory, install_panic_invoke_guard};`
- [x] 4.2 `modules/actor-adaptor-std/src/std/actor/panic_invoke_guard.rs` を新規作成
  - `pub struct PanicInvokeGuard;` + `impl InvokeGuard for PanicInvokeGuard`
    - `wrap_receive` 内で `std::panic::catch_unwind(AssertUnwindSafe(|| call()))`
    - `Ok(Ok(()))` / `Ok(Err(e))` は素通し
    - `Err(panic_payload)` は `Err(ActorError::Escalate(ActorErrorReason::new(format!("panic: {msg}"))))` に変換（panic payload から `&str` / `String` を取り出す）
  - `pub struct PanicInvokeGuardFactory;` + `impl InvokeGuardFactory for PanicInvokeGuardFactory { fn build(&self) -> ArcShared<Box<dyn InvokeGuard>> { ArcShared::new(Box::new(PanicInvokeGuard)) } }`
  - `pub fn install_panic_invoke_guard(config: ActorSystemConfig) -> ActorSystemConfig { config.with_invoke_guard_factory(ArcShared::new(Box::new(PanicInvokeGuardFactory))) }`
- [x] 4.3 `modules/actor-adaptor-std/src/std.rs` に `pub mod actor;` を追加
- [x] 4.4 既存テスト `sp_h1_5_t1..t4` が全 passing することを確認
  - `rtk cargo test -p fraktor-actor-adaptor-std-rs --test sp_h1_5_panic_guard`
- [x] 4.5 std adaptor の既存 integration test（panic が supervisor エスカレーションされる観測）を 1 件追加
  - 例: `modules/actor-adaptor-std/tests/sp_h1_5_system_escalation.rs` で `install_panic_invoke_guard` 経由で構築した ActorSystem 下で child actor の `receive` が panic したときに parent が `ActorError::Escalate` を supervisor directive として観測すること
- [x] 4.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 5. 検証

- [x] 5.1 `rtk cargo test -p fraktor-actor-core-rs` passing（kernel 側 NoopInvokeGuard の挙動維持、config 経由の factory 取得が全 spawn 経路で機能）
- [x] 5.2 `rtk cargo test -p fraktor-actor-adaptor-std-rs` passing（SP-H1.5 t1..t4 含む、4.5 の system escalation test も含む）
- [x] 5.3 `rtk cargo check --workspace --no-default-features` で no_std target での core のビルドが通ること（core 層が std 非依存）
- [x] 5.4 section 1〜4 の各末尾で `./scripts/ci-check.sh ai dylint` を実行済みであることを再確認（本項目で追加実行する必要はない。`ai all` に dylint が含まれるため section 6.5 で最終実行される）

## 6. 品質ゲート（マージ前 MUST 条件）

本 change が proposal の 4 原則を満たしていることをマージ前に以下の項目で機械的に裏取りする。1 つでも fail したら該当作業に戻す。

### 6.1 原則 2 (本質的な設計を選ぶ) のゲート

- [x] 6.1.1 `MessageInvokerPipeline` に generic parameter `G: InvokeGuard` が伝播していないこと
  - `rtk grep -n "MessageInvokerPipeline<" modules/actor-core/src/` で generic 付きの宣言・参照が 0 件
  - `guard: ArcShared<Box<dyn InvokeGuard>>` フィールドを持つ非ジェネリック定義のみ存在する
- [x] 6.1.2 `Spawn<Sp, Si, Tk>` 階層に `G: InvokeGuard` 等の追加 generic parameter が伝播していないこと
- [x] 6.1.3 `InvokeGuard` trait が dyn-compatible を維持していること
  - `let _: ArcShared<Box<dyn InvokeGuard>>;` がコンパイルできる
  - `wrap` は `Self: Sized` 付き default method として `dyn InvokeGuard` からは呼べない（object safety 保持）
- [x] 6.1.4 段階的妥協の痕跡がないこと
  - `rtk grep -rn "TODO\|FIXME\|HACK\|XXX\|暫定\|workaround" modules/actor-core/src/core/kernel/actor/invoke_guard* modules/actor-adaptor-std/src/std/actor/` で本 change 由来の TODO / workaround がないこと

### 6.2 原則 3 (後方互換性を保つコードを書かない) のゲート

- [x] 6.2.1 `MessageInvokerPipeline::new()` (引数無しの旧 constructor) が workspace 全体で 0 件
  - `rtk grep -rn "MessageInvokerPipeline::new\b(" modules/` で `new_with_guard(` を除外した引数無し呼び出しが 0 件
- [x] 6.2.2 後方互換のための wrapper / alias / re-export が 0 件
  - `pub fn new() -> Self { Self::new_with_guard(default_guard) }` のような暫定 constructor を書かない
  - `rtk grep -rn "legacy\|compat\|deprecated\|backwards" modules/actor-core/src/core/kernel/actor/invoke_guard* modules/actor-adaptor-std/src/std/actor/` で互換コードが 0 件
- [x] 6.2.3 本 change で追加した module に未使用 trait / 未使用 method / 未使用 variant がないこと
  - `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` と `modules/actor-core/src/core/kernel/actor/invoke_guard/*`、`modules/actor-adaptor-std/src/std/actor/*` に `InvokeGuardExt` 等の未使用拡張 trait や暫定 API を残さない
  - repo-wide な `-D dead_code` は既存 test 群の未解消項目を含むため、本 change の完了条件には含めない

### 6.3 原則 4 (no_std core + std adaptor 分離) のゲート

- [x] 6.3.1 本 change で追加した core 側 `invoke_guard` 実装に `std::*` / `std::panic` / `catch_unwind` を持ち込まないこと
  - `modules/actor-core/src/core/kernel/actor/invoke_guard.rs` および `modules/actor-core/src/core/kernel/actor/invoke_guard/*` に `std::*` / `std::panic` / `catch_unwind` が存在しないこと
  - repo 既存の `src/` 配下 test module にある `std` 利用は本 change の判定対象外とする
- [x] 6.3.2 `PanicInvokeGuard` / `PanicInvokeGuardFactory` / `install_panic_invoke_guard` が `modules/actor-adaptor-std/src/std/actor/` にのみ存在し、core 側に re-export されていないこと
- [x] 6.3.3 `cfg-std-forbid` dylint が違反を検出しないこと（下記 6.5.1 に含まれる）
- [x] 6.3.4 `rtk cargo check --workspace --no-default-features` で no_std target でのビルドが通ること（core 層が std 非依存）

### 6.4 Pekko 参照実装 parity のゲート

- [x] 6.4.1 Pekko dispatcher の「`try/catch` で `receiveMessage` を包み `Throwable` を `ActorFailure` としてエスカレーション」という意味論が `PanicInvokeGuard::wrap_receive` の `catch_unwind` + `ActorError::Escalate` 変換で再現されていること
- [x] 6.4.2 lifecycle hooks (`pre_start` / `post_stop` / `pre_restart` / `post_restart`) を panic guard で包まない判断が proposal「非目標」に明示されていること（Phase A3 送り）

### 6.5 CI / lint の final ゲート

- [x] 6.5.1 `./scripts/ci-check.sh ai all` が最終動作確認として exit 0（内部で dylint / cargo test / clippy / fmt を全件実行。8 custom lint 全 pass: mod-file / module-wiring / type-per-file / tests-location / use-placement / rustdoc / cfg-std-forbid / ambiguous-suffix。TAKT ルール上、このゲートは change のマージ直前にのみ実行）
