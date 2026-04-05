## Why

現在の dispatcher まわりは、公開概念・選択 API・runtime 実体の境界が曖昧である。

- 公開面では `DispatcherConfig` が主語になっている
- runtime primitive である `DispatchExecutor` / `DispatcherShared` が利用者の目に触れる
- `PinnedDispatcher` が policy ではなく config factory として扱われている
- named dispatcher selection、parent 継承、blocking 用予約 ID といった選択意味論が `Config` 主体の設計に埋もれている

この change の目的は、dispatcher まわりを「どう実装するか」ではなく「何が成立していなければならないか」で定義し直すことである。

## What Must Hold

- dispatcher の公開抽象は `Dispatcher` / `DispatcherProvider` を主語とする
- actor / system の dispatcher 選択は registry entry と selector 意味論で説明できる
- `Props` は provider や settings を直接保持しない
- `PinnedDispatcher` は dedicated lane policy として観測できる
- std adapter の公開面は policy family と helper に限定される
- default dispatcher、blocking dispatcher、parent 継承の意味論が仕様として明示される
- bootstrap 文脈で `same-as-parent` が指定された場合は reserved default entry へ解決される
- Pekko 互換 public identifier と kernel registry id の対応が具体的に固定される
- 利用者が dispatcher を明示登録しなくても system default config が default dispatcher entry を提供する
- 既存 capability のうち dispatcher redesign と衝突するものは、この change 内で同時に更新される

## What Must Not Hold

- `DispatcherConfig` を dispatcher public concept の主語として残してはならない
- runtime backend 名を public policy 名へ昇格させてはならない
- `Props` に provider / settings / runtime handle を直接持たせてはならない
- `PinnedDispatcher` を config factory や dispatch ごとの thread spawn として扱ってはならない
- `DefaultDispatcher` の feature 差を runtime fallback で吸収してはならない
- archived spec に `DispatcherConfig` 前提の要件を残したまま redesign を完了扱いにしてはならない

## Capabilities

### Modified Capabilities
- `dispatch-executor-unification`: executor 系を internal backend primitive としてのみ扱う
- `actor-std-adapter-surface`: std adapter 公開面から config / wrapper 主体の façade を除去する
- `actor-system-default-config`: default dispatcher 要件を `DispatcherConfig` 前提から切り離す

### New Capabilities
- `dispatcher-trait-provider-abstraction`: dispatcher family を trait/provider と selection semantics で定義する

## Impact

- 影響コード:
  - `modules/actor/src/core/kernel/dispatch/dispatcher/*`
  - `modules/actor/src/core/kernel/actor/props/*`
  - `modules/actor/src/core/kernel/actor/setup/*`
  - `modules/actor/src/core/typed/dispatchers*`
  - `modules/actor-adaptor/src/std/dispatch/*`
- 影響 API:
  - `Dispatcher`
  - `DispatcherProvider`
  - `Dispatchers`
  - `ActorSystemConfig`
  - `Props`
  - `PinnedDispatcher`
- 互換性:
  - 後方互換は不要
  - ただし、default / blocking / same-as-parent の意味論は未定義のまま削除してはならない
