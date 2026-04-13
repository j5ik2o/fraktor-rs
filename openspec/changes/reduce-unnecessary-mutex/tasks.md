## 1. 各候補の write-once 検証

- [ ] 1.1 `MiddlewareShared` のアクセスパターンを検証する（初期化後に `with_write` / `with_lock` で値を変更している箇所がないか）
- [ ] 1.2 `ActorRefProviderHandleShared` のアクセスパターンを検証する
- [ ] 1.3 `ExecutorShared` のアクセスパターンを検証する（`trampoline` フィールドは除外 — mutable state）
- [ ] 1.4 `MessageDispatcherShared` のアクセスパターンを検証する
- [ ] 1.5 `DeadLetterShared` のアクセスパターンを検証する

## 2. 検証合格した候補を spin::Once に置換

- [ ] 2.1 検証合格した各 `*Shared` 型を `spin::Once<T>` に置換する
- [ ] 2.2 各型のコンストラクタを更新する（`spin::Once::initialized(value)` を使用）
- [ ] 2.3 各型の read アクセスサイトを `get()` ベースに更新する

## 3. 検証

- [ ] 3.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [ ] 3.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [ ] 3.3 `./scripts/ci-check.sh` が全パスすることを確認する
- [ ] 3.4 `cargo bench --features tokio-executor -p fraktor-actor-adaptor-std-rs` で before/after を比較する
