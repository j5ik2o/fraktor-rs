## ADDED Requirements

### Requirement: dispatcher public abstraction は trait/provider 中心でなければならない

dispatcher の public abstraction は `Dispatcher` trait と `DispatcherProvider` trait を中心に定義されなければならない。設定オブジェクトや runtime primitive を public concept の主語にしてはならない。

#### Scenario: public surface に dispatcher trait/provider が現れる
- **WHEN** `core::dispatch::dispatcher` の公開面を確認する
- **THEN** `Dispatcher` trait が公開 API として存在する
- **AND** `DispatcherProvider` trait が公開 API として存在する
- **AND** `DispatcherSettings` が provider へ渡す settings snapshot として存在する

#### Scenario: public surface は config / runtime primitive を主語にしない
- **WHEN** dispatcher 関連の public API を確認する
- **THEN** `DispatcherConfig`、`DispatcherShared`、`DispatchExecutor`、`DispatchExecutorRunner` は public concept の主語として扱われない

### Requirement: dispatcher selection は registry entry と selector 意味論で行われる

actor / system の dispatcher selection は、registry entry と selector 意味論に基づいて行われなければならない。`Props` が provider や settings を直接保持してはならない。

#### Scenario: Props は dispatcher 選択情報だけを保持する
- **WHEN** `Props` の dispatcher selection API を確認する
- **THEN** `Props` は dispatcher id を指定できる
- **AND** same-as-parent の選択を表現できる
- **AND** provider や settings を direct に保持する API は存在しない

#### Scenario: ActorSystemConfig は registry entry を登録する
- **WHEN** `ActorSystemConfig` の dispatcher registration API を確認する
- **THEN** dispatcher id に対して provider と settings を束ねた registry entry を登録できる
- **AND** bootstrap は registry entry を `DispatcherProvisionRequest` に固定化して actor 用 dispatcher を provision する

### Requirement: default / blocking / typed selector の意味論は固定される

default dispatcher、blocking dispatcher、typed selector の対応関係は redesign 後も一意に解決できなければならない。

#### Scenario: default dispatcher は予約済み entry として解決できる
- **WHEN** default dispatcher を選択する
- **THEN** system は予約済みの default dispatcher entry を解決できる
- **AND** default dispatcher の存在有無が実装依存にならない

#### Scenario: Pekko default identifier は reserved default entry へ正規化される
- **WHEN** typed 側または config-based selector で `pekko.actor.default-dispatcher` を指定する
- **THEN** その指定は kernel registry id `"default"` へ正規化される
- **AND** 別の internal id や feature 依存の id へ解決されない

#### Scenario: blocking dispatcher の予約 ID は redesign 後も解決できる
- **WHEN** blocking workload 用 dispatcher id を選択する
- **THEN** system はその予約済み id を registry から解決できる
- **AND** feature 差によって別の意味へ暗黙に変化しない

#### Scenario: typed selector は kernel dispatcher id へ正規化できる
- **WHEN** typed 側で default / blocking / config-based selector を使う
- **THEN** selector は kernel registry が解決できる dispatcher id へ正規化される
- **AND** typed / untyped で同じ selector が異なる dispatcher へ解決されない

#### Scenario: internal dispatcher identifier を公開し続ける場合は default entry へ解決される
- **WHEN** `Dispatchers::INTERNAL_DISPATCHER_ID` を public identifier として公開し続ける
- **THEN** その identifier は kernel registry id `"default"` への別名として解決される
- **AND** 専用の internal dispatcher entry を暗黙に要求しない

### Requirement: same-as-parent は独立した選択意味論として扱われる

same-as-parent は default dispatcher の別名ではなく、親 actor の dispatcher selection 結果を継承する独立の意味論として扱われなければならない。

#### Scenario: child actor は親の dispatcher selection 結果を継承する
- **WHEN** child actor が same-as-parent を選択して起動する
- **THEN** child は parent actor の dispatcher selection 結果を継承する
- **AND** parent 継承は actor 起動時に解決される

#### Scenario: bootstrap 文脈の same-as-parent の扱いは固定される
- **WHEN** 親 actor が存在しない bootstrap 文脈で same-as-parent が指定される
- **THEN** same-as-parent は reserved default dispatcher entry へ解決される
- **AND** エラー化や別 entry への解決へ委ねられない

### Requirement: PinnedDispatcher は dedicated lane policy を提供する

`PinnedDispatcher` は config factory ではなく dedicated lane policy を提供する dispatcher provider でなければならない。

#### Scenario: pinned policy が actor lifecycle に従って専用 lane を持つ
- **WHEN** `PinnedDispatcher` で actor を起動する
- **THEN** その actor は他 actor と lane を共有しない
- **AND** actor 停止時に専用 lane も停止・解放される

#### Scenario: pinned policy は end-to-end に選択できる
- **WHEN** `ActorSystemConfig` に `"pinned"` dispatcher entry を登録し、`Props` が `"pinned"` を選択する
- **THEN** bootstrap は `PinnedDispatcher` provider を解決して actor 用 dispatcher を provision する
- **AND** actor は pinned policy で起動する
