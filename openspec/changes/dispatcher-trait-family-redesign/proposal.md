## Why

現在の dispatcher 設計は、概念の主語と型の主語がずれている。

利用者と設計者が語りたい概念は `Dispatcher` であるにもかかわらず、公開 API の中心には `DispatcherConfig` が置かれ、実装境界では `DispatchExecutor` と `DispatcherShared` が前面に現れている。この結果、dispatcher policy を追加・選択する設計になっておらず、`PinnedDispatcher` も「policy」ではなく「config factory」としてしか表現されていない。

この構造は以下の問題を生む。

- `Dispatcher` という概念が trait として表現されていない
- `DispatcherConfig` が actor / system 接続点の主語になり、設定オブジェクトが責務を吸い込んでいる
- `PinnedDispatcher` を含む `XXXDispatcher` family を policy として拡張しにくい
- Tokio / embassy など runtime ごとの差分を「dispatcher policy」と「backend primitive」に分離できていない
- 後方互換不要な段階にもかかわらず、legacy abstraction を温存しやすい

この change では、dispatcher を trait 中心の設計へ置き換え、`PinnedDispatcher` を含む dispatcher family を policy として拡張できる構造へ再設計する。

## What Changes

- `Dispatcher` を公開概念として trait で導入する
- `DispatcherProvider` を導入し、actor / system へ dispatcher を供給する正式な境界とする
- `Dispatchers` registry は `DispatcherConfig` ではなく dispatcher provider を保持する
- `ActorSystemConfig` と `Props` は config ではなく dispatcher provider / dispatcher id を主語にする
- `PinnedDispatcher` を config factory ではなく dispatcher family の一員として再定義する
- `DispatchExecutor`、`DispatchExecutorRunner`、`DispatcherShared`、既存 `DispatcherConfig` は legacy abstraction とみなし、この change 内で公開面から除去する
- adapter は runtime-specific dispatcher family の実装を提供し、`core` の `ActorSystem` や `ClusterApi` を再ラップしない

## Capabilities

### Modified Capabilities
- `dispatch-executor-unification`: `DispatchExecutor` を public abstraction の中心から外し、dispatcher trait/provider 中心へ置き換える
- `actor-std-adapter-surface`: std adapter の公開面を runtime-specific dispatcher family と helper に限定し、config/wrapper 主体の façade を除去する

### New Capabilities
- `dispatcher-trait-provider-abstraction`: dispatcher family を trait/provider ベースで定義し、`PinnedDispatcher` を含む `XXXDispatcher` policy を追加可能にする

## Impact

- 影響コード:
  - `modules/actor/src/core/kernel/dispatch/dispatcher/*`
  - `modules/actor/src/core/kernel/actor/props/*`
  - `modules/actor/src/core/kernel/actor/setup/*`
  - `modules/actor-adaptor/src/std/dispatch/*`
  - dispatcher を選択・登録している showcase / cluster / remote まわり
- 影響 API:
  - `Dispatcher`
  - `DispatcherProvider`
  - `Dispatchers`
  - `ActorSystemConfig`
  - `Props`
  - `PinnedDispatcher`
- 互換性:
  - 後方互換は不要
  - 互換ブリッジは導入しない
  - 旧 abstraction は同一 change で除去する
