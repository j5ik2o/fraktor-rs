## 1. Core Public Abstraction の置換

- [x] 1.1 `core::dispatch::dispatcher` に `Dispatcher`、`DispatcherProvider`、`DispatcherSettings`、`DispatcherProvisionRequest` を追加し、dispatcher public abstraction を trait/provider 主体へ置き換える
- [x] 1.2 `DispatcherConfig`、`DispatcherShared`、`DispatchExecutor`、`DispatchExecutorRunner` が dispatcher public concept の主語として露出しないように、公開面の re-export と参照経路を整理する
- [x] 1.3 executor 系を internal backend primitive として残す場合でも、actor / system の dispatcher selection API が executor 型に依存しないことを確認する

## 2. Registry と Selection Semantics の再構成

- [x] 2.1 `Dispatchers` を provider + settings を束ねた registry entry を保持する構造へ置き換える
- [x] 2.2 `ActorSystemConfig` を dispatcher registry entry 登録 API に更新し、`Props` が provider / settings 実体を保持しない構造へ揃える
- [x] 2.3 actor bootstrap を registry entry から `DispatcherProvisionRequest` を固定化して actor 用 dispatcher を provision する流れへ更新する
- [x] 2.4 `same-as-parent` を独立した選択意味論として実装し、親 actor がある場合は親の dispatcher selection 結果を継承する
- [x] 2.5 bootstrap 文脈で `same-as-parent` が指定された場合は reserved default dispatcher entry へ解決するように固定する

## 3. Reserved ID と Typed Selector の固定

- [x] 3.1 reserved default dispatcher entry と blocking dispatcher entry を redesign 後も一意に解決できるようにする
- [x] 3.2 typed `Default`、typed `Blocking`、`FromConfig(\"pekko.actor.default-dispatcher\")` の各 selector が設計どおりの kernel registry id へ正規化されるようにする
- [x] 3.3 `Dispatchers::INTERNAL_DISPATCHER_ID` を公開し続ける場合は kernel registry id `"default"` の別名として解決されるようにする
- [x] 3.4 typed / untyped で同じ selector が異なる dispatcher へ解決されないことを tests で固定する

## 4. Dispatcher Policy Family の置換

- [x] 4.1 std adapter に `DefaultDispatcher`、`PinnedDispatcher`、`BlockingDispatcher` の policy family を揃える
- [x] 4.2 `PinnedDispatcher` を dedicated lane policy として実装し、actor ごとに lane を共有せず、actor lifecycle に追従して停止・解放されることを確認する
- [x] 4.3 `tokio-executor` feature 無効時に `DefaultDispatcher` を thread backend へ fallback しないことを公開面と tests で固定する
- [x] 4.4 `DispatcherConfig`、`DispatchExecutor`、`DispatchExecutorRunner`、`TokioExecutor`、`ThreadedExecutor` が std adapter の public policy surface に現れないようにする

## 5. Default Config と呼び出し側追随

- [x] 5.1 `ActorSystemConfig::default()` 相当の system default config が、caller の明示登録なしで reserved default dispatcher entry を提供するようにする
- [x] 5.2 dispatcher 未指定の actor が default dispatcher entry から起動することを classic / typed の両方で確認する
- [x] 5.3 showcase / bench / cluster / remote を新しい dispatcher selection API へ追随させ、`DispatcherConfig` 主体の呼び出しを除去する

## 6. Legacy Surface の削除と検証

- [x] 6.1 旧 `DispatcherConfig` 主体 API、config factory としての `PinnedDispatcher`、dispatcher public surface 上の legacy re-export を削除する
- [x] 6.2 dispatcher redesign と衝突する archived capability 更新内容を確認し、archive 時に `actor-system-default-config` を `DispatcherConfig::default()` 非依存の要件へ置き換えられる状態にする
- [x] 6.3 dispatcher 関連 tests、typed selector tests、std adapter surface tests を更新する
- [x] 6.4 最終確認として `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認する
