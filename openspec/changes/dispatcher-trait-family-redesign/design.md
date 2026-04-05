## Context

dispatcher redesign で必要なのは、構成要素の作り方の指示ではなく、公開面・選択面・runtime 面で満たされるべき境界条件の固定である。

この change は次の曖昧さを解消対象とする。

- dispatcher public abstraction が `DispatcherConfig` 主体になっている
- `DispatchExecutor` / `DispatcherShared` / `DispatchExecutorRunner` が public surface に近すぎる
- `PinnedDispatcher` の意味論が dedicated lane policy として固定されていない
- default / blocking / same-as-parent / typed selector の対応関係が仕様化されていない
- archived capability に `DispatcherConfig` 前提が残っている

## Design Constraints

### 1. Public Abstraction

#### 満たすべき条件

- dispatcher public abstraction は `Dispatcher` trait と `DispatcherProvider` trait を中心に説明できなければならない
- `DispatcherSettings` は provider へ渡される確定済み snapshot として扱われなければならない
- actor / system 利用者が dispatcher を選択するとき、公開概念として触れるのは policy 名、dispatcher id、selector 意味論のいずれかに限られなければならない

#### 満たしてはいけない条件

- `DispatcherConfig`、`DispatcherShared`、`DispatchExecutor`、`DispatchExecutorRunner` を public concept の主語にしてはならない
- runtime backend primitive を使わないと dispatcher が説明できない API であってはならない

### 2. Registry And Selection Semantics

#### 満たすべき条件

- system 側には dispatcher registry entry が存在し、各 entry は少なくとも provider と settings を束ねなければならない
- actor 起動時の dispatcher 決定は、registry entry を起点に一意に説明できなければならない
- `Props` は dispatcher 選択情報のみを保持し、provider や settings 実体を保持してはならない
- dispatcher 選択には少なくとも次の 3 種類の意味論が存在しなければならない
  - 明示的な dispatcher id 選択
  - default dispatcher への解決
  - same-as-parent による親 dispatcher 継承

#### 満たしてはいけない条件

- provider を bootstrap 側へ裸で返し、そこで settings merge や優先順位判断を再度行う設計であってはならない
- `Props` が provider / settings / runtime handle を直接保持する設計であってはならない
- same-as-parent を default id への単純な別名として潰してはならない

### 3. Reserved IDs And Typed Mapping

#### 満たすべき条件

- registry には default dispatcher を識別する予約済み entry が存在しなければならない
- blocking workload 用の予約済み dispatcher id は redesign 後も解決可能でなければならない
- typed 側の dispatcher selector は redesign 後も dispatcher id へ正規化できなければならない
- Pekko 互換の public identifier を保持する場合、その public identifier から kernel registry id への対応関係は仕様で明示されなければならない
- redesign で維持する identifier 対応は少なくとも次に固定されなければならない
  - typed `Default` は kernel registry id `"default"` へ解決される
  - typed `Blocking` は kernel registry id `"pekko.actor.default-blocking-io-dispatcher"` へ解決される
  - `FromConfig("pekko.actor.default-dispatcher")` は kernel registry id `"default"` へ正規化される
  - `Dispatchers::INTERNAL_DISPATCHER_ID` を公開し続ける場合は kernel registry id `"default"` への別名として解決される

#### 満たしてはいけない条件

- default dispatcher の存在有無が実装依存になってはならない
- typed / untyped で同じ selector が異なる dispatcher へ解決されてはならない
- feature 差によって reserved id の意味が暗黙に変わってはならない

### 4. Parent Inheritance

#### 満たすべき条件

- same-as-parent を選んだ actor は、親 actor が存在する場合に親の dispatcher selection 結果を継承しなければならない
- 親が存在しない bootstrap 文脈で same-as-parent が指定された場合は reserved default entry へ解決されなければならない
- parent 継承は actor 起動時に解決される意味論として扱われなければならない

#### 満たしてはいけない条件

- same-as-parent の扱いを未定義のまま public API から削除してはならない
- parent 継承時に provider や settings の追加 merge 規則をその場で再発明してはならない

### 5. Pinned Policy

#### 満たすべき条件

- `PinnedDispatcher` は dedicated lane policy を表さなければならない
- dedicated lane は actor 単位で分離され、他 actor と共有されてはならない
- lane の存続期間は actor lifecycle に追従しなければならない
- actor 停止後に lane が停止・解放されることを仕様で要求しなければならない

#### 満たしてはいけない条件

- `PinnedDispatcher` を config factory として定義してはならない
- dispatch ごとの thread spawn を pinned semantics と見なしてはならない
- registry や actor system が pinned lane を共有・再利用する前提を置いてはならない

### 6. Std Adapter Surface

#### 満たすべき条件

- std adapter の dispatcher 公開面は policy family と std 固有 helper のみに限定されなければならない
- public policy 名としては `DefaultDispatcher`、`PinnedDispatcher`、`BlockingDispatcher` を用い、backend 名は internal detail に留めなければならない
- std adapter は core の `ActorSystem` を包み直した façade を公開してはならない

#### 満たしてはいけない条件

- `DispatcherConfig`、`DispatchExecutor`、`DispatchExecutorRunner`、`TokioExecutor`、`ThreadedExecutor` を std adapter の public policy surface に出してはならない
- `std.rs` に leaf helper の中継用 wrapper を追加してはならない

### 7. Feature Gating

#### 満たすべき条件

- `DefaultDispatcher` の提供有無は `tokio-executor` feature によって明示的に切り替わらなければならない
- feature 差により提供されない policy は「存在しない」として扱われなければならない

#### 満たしてはいけない条件

- `tokio-executor` 無効時に thread backend へ fallback して `DefaultDispatcher` を同名のまま提供してはならない
- feature 差を runtime 分岐で隠蔽してはならない

### 8. Archived Capability Alignment

#### 満たすべき条件

- archived capability のうち dispatcher redesign と衝突するものは、この change 内で同時に更新されなければならない
- とくに actor system の default dispatcher 要件は、`DispatcherConfig::default()` 前提ではなく「default dispatcher entry が解決可能であること」で記述し直されなければならない
- `ActorSystemConfig::default()` 相当の system default config は、caller の明示登録なしで reserved default dispatcher entry を提供しなければならない

#### 満たしてはいけない条件

- redesign 完了後も archived spec に `DispatcherConfig` 前提の requirement を残してはならない
- change spec と archived spec が同時に成立しない状態を受け入れてはならない

## Acceptance Checklist

- dispatcher public surface が trait/provider 主体で説明できる
- registry / default / blocking / same-as-parent の意味論が仕様として明示されている
- `Props` が provider / settings 実体を保持しない
- `PinnedDispatcher` が dedicated lane policy として要求されている
- std adapter の公開面から config / executor / wrapper 主語が排除されている
- `actor-system-default-config` との整合が change に含まれている
