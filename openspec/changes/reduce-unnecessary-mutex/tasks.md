## 1. 対象候補の write-once 検証（完了）

- [x] 1.1 `MiddlewareShared` のアクセスパターンを検証する → **除外**: `with_write` で `before_user`/`after_user`（`&mut self`）が毎メッセージ呼ばれる
- [x] 1.2 `ActorRefProviderHandleShared` のアクセスパターンを検証する → **除外**: `register_temp_actor`/`unregister_temp_actor` 等の変更操作が ongoing
- [x] 1.3 `ExecutorShared` のアクセスパターンを検証する → **除外**: `Executor::execute(&mut self)` が毎タスクで呼ばれる
- [x] 1.4 `MessageDispatcherShared` のアクセスパターンを検証する → **除外**: `attach`/`detach`/`dispatch` 等が hot path で実行
- [x] 1.5 `DeadLetterShared` のアクセスパターンを検証する → **除外**: `record_send_error`/`record_entry` で追記
- [x] 1.6 actor-core 全体（約 35 型）を網羅的に再調査する → 新規 write-once 候補を 2 つ発見

## 2. 検証合格した候補を spin::Once に置換

- [x] 2.1 `CoordinatedShutdown.reason` を `spin::Once<CoordinatedShutdownReason>` に置換する
  - `run()` の `self.reason.with_write(...)` → `self.reason.call_once(|| reason)`
  - `shutdown_reason()` の `self.reason.with_read(Clone::clone)` → `self.reason.get().cloned()`
  - コンストラクタの `SharedLock::new_with_driver::<DefaultMutex<_>>(None)` → `spin::Once::new()`
- [x] 2.2 `ContextPipeWakerHandleShared.inner` を `spin::Once<ContextPipeWakerHandle>` に置換する
  - コンストラクタで `spin::Once::initialized(handle)` を使用
  - `wake()` の `inner.with_lock(|guard| ...)` → `inner.get().expect(...)`
  - `Clone` impl と `from_shared_lock` を削除（外部呼び出しなし、ArcShared でラップ済み）

## 3. 検証

- [x] 3.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [x] 3.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [x] 3.3 `./scripts/ci-check.sh` が全パスすることを確認する
- [x] 3.4 `cargo bench --features tokio-executor -p fraktor-actor-adaptor-std-rs` で before/after を比較する
  - `default_dispatcher_baseline/single_actor_batch_100`: **-5.5%** (459.70µs, p=0.00) Performance improved
  - `default_dispatcher_baseline/single_actor_batch_1000`: **-5.5%** (4.67ms, p=0.00) Performance improved
  - `balancing_dispatcher/team_4_batch_1000`: 有意差なし (p=0.09)
