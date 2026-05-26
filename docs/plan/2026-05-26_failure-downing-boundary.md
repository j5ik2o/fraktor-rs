# failure / downing boundary note

## 決定

Failure detector と membership coordination が扱う suspect / unreachable は observation であり、member departure input ではない。

`cluster-core` は `DowningProvider` を core-defined decision port として所有する。explicit down command と failure observation は `DowningInput` として strategy に渡され、strategy が `DowningDecision::Down` を返した場合だけ provider-neutral な departure input に進む。

## 最小 contract

- `DowningInput::ExplicitDown` は explicit down command の入口。
- `DowningInput::FailureObservation` は failure detector / membership 由来の availability observation の入口。
- `DowningDecision::Down` は active topology から authority を外す判断。
- `DowningDecision::Keep` / `DowningDecision::Defer` は active topology を保持する判断。
- Grain runtime は topology update の `left` / `dead` によって stale activation と PID cache を invalidation する。phi value、suspect timer、SBR details は inspect しない。

## 対象外

Split Brain Resolver、reachability matrix、rebalance、remembered entities、in-flight drain はこの change では実装しない。
