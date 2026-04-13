# actor-system-default-config Specification

## Purpose
`ActorSystem` のデフォルト dispatcher 要件を public config 型ではなく、system が解決可能な reserved default dispatcher entry の存在として定義する。
## Requirements
### Requirement: actor system の default dispatcher は public config 型なしで解決できる

actor system の default dispatcher 要件は、`DispatcherConfig::default()` のような public config 型ではなく、kernel registry に登録された reserved default `MessageDispatcherConfigurator` の存在として定義されなければならない (MUST)。`Dispatchers::resolve("default")` は常に `Some(MessageDispatcherShared)` を返さなければならない (MUST)。

#### Scenario: dispatcher 未指定の actor は default entry から解決される

- **WHEN** actor が dispatcher を明示せずに起動する
- **THEN** bootstrap は `Dispatchers` registry に登録された `default` configurator を `MessageDispatcherShared` として解決する
- **AND** caller が `ActorSystemConfig::with_dispatcher_configurator` を呼び出さなくても default dispatcher が適用される

#### Scenario: system default config は caller の明示登録なしで default configurator を提供する

- **WHEN** caller が dispatcher registry を追加設定せずに `ActorSystemConfig::default()` を使う
- **THEN** system は reserved default configurator (`InlineExecutor` ベースの `DefaultDispatcherConfigurator`) を保持して起動できる
- **AND** caller に default dispatcher の明示登録を要求しない (`Dispatchers::ensure_default_inline()` が seeding を担当する)

#### Scenario: default dispatcher 要件は configurator registry ベースで説明される

- **WHEN** actor system の default dispatcher 要件を確認する
- **THEN** その要件は default `MessageDispatcherConfigurator` と `Dispatchers` registry の存在で説明される
- **AND** `DispatcherConfig::default()` の存在を前提にしない
- **AND** `DispatcherProvider` / `DispatcherRegistryEntry` のような旧 API の存在を前提にしない

#### Scenario: feature 差は runtime fallback ではなく提供面の差として現れる

- **WHEN** `tokio-executor` feature の有無で std adapter の executor 提供面を確認する
- **THEN** std adapter の `TokioExecutor` / `TokioExecutorFactory` の提供有無は feature で明示される
- **AND** thread backend への暗黙 fallback で default 要件を満たしたことにしない
- **AND** core 層の `DefaultDispatcher` 自体は feature によらず常に存在し、std 側は executor だけを差し込む構造である

### Requirement: actor system は shared runtime override seam を持たず builtin spin backend で shared runtime surface を seed しなければならない

actor system は、多数の型別 `*SharedFactory` や単一の lock factory seam を使った shared runtime override を持たず、builtin spin backend で default dispatcher と shared runtime surface を seed しなければならない（MUST）。`ActorSystemConfig` / `ActorSystemSetup` が `with_shared_factory(...)` や `with_lock_factory(...)` のような shared runtime override API を公開してはならない（MUST NOT）。

#### Scenario: default config は builtin spin backend で shared runtime surface を seed する
- **WHEN** caller が `ActorSystemConfig::default()` を使う
- **THEN** actor system は builtin spin backend を使って default dispatcher と shared runtime surface を構築する
- **AND** bootstrap path に shared runtime override 未設定の穴は存在しない

#### Scenario: actor system config は shared runtime override API を持たない
- **WHEN** actor system config の公開 API を確認する
- **THEN** caller は dispatcher / mailbox / extension などの runtime setting は設定できる
- **AND** shared runtime surface の backend を差し替える API は存在しない
- **AND** `with_shared_factory(...)` や `with_lock_factory(...)` は存在しない

#### Scenario: config API は nongeneric のまま保たれる
- **WHEN** actor system config の公開 API を確認する
- **THEN** `ActorSystem` / `ActorRef` / typed system の公開型に generic parameter は追加されない
- **AND** backend 選択のための公開 generic 型や runtime factory seam は導入されない

### Requirement: actor system の lock family 設定は default から spawn path まで一貫して反映されなければならない

actor system が選択した lock family は、default dispatcher の seed、spawn path、および shared runtime surface まで一貫して反映されなければならない（MUST）。`with_lock_provider(...)` で上書きした provider が一部の bootstrap path にしか反映されない状態を許してはならない（MUST NOT）。

#### Scenario: default config は既定 provider family で shared runtime surface を seed する
- **WHEN** caller が明示的な lock provider override なしで `ActorSystemConfig::default()` を使う
- **THEN** actor system は既定 provider family で dispatcher と関連 shared wrapper を seed する
- **AND** bootstrap path に provider 未設定の穴が存在しない
- **AND** actor-core bootstrap 内で `new_with_builtin_lock(...)` や `new_with_driver::<SpinSync*>` による silent bypass が存在しない

#### Scenario: custom provider override は spawn path まで貫通する
- **WHEN** caller が `ActorSystemConfig::with_lock_provider(...)` で custom provider を設定する
- **THEN** actor system の spawn / bootstrap path はその provider を使って shared runtime surface を構築する
- **AND** 既定 provider family に戻る silent fallback は存在しない
- **AND** provider-sensitive な runtime-owned bootstrap state も同じ provider family を受け取る

### Requirement: actor system default config は default `ActorLockProvider` を seed し、明示 override を許可する

`ActorSystemConfig::default()` は、caller が追加設定しなくても actor system hot path を構築できる default `ActorLockProvider` を seed しなければならない（MUST）。同時に caller は system ごとに明示 override できなければならない（MUST）。

#### Scenario: default config は追加設定なしで provider を持つ
- **WHEN** caller が `ActorLockProvider` を明示設定せずに `ActorSystemConfig::default()` を使う
- **THEN** actor system は default `ActorLockProvider` を使って起動できる
- **AND** caller に std adapter helper や build-time generic parameter を要求しない

#### Scenario: caller は system ごとに provider を上書きできる
- **WHEN** caller が `ActorSystemConfig` に対して custom `ActorLockProvider` を設定して起動する
- **THEN** その system の hot path は custom provider で構築される
- **AND** 他の actor system の default provider 構成には影響しない

#### Scenario: setup facade からも同じ override ができる
- **WHEN** caller が `ActorSystemSetup` を使って custom `ActorLockProvider` を設定する
- **THEN** 最終的な `ActorSystemConfig` にはその provider が保持される
- **AND** `ActorSystemConfig` へ直接設定した場合と同じ意味論で扱われる

#### Scenario: override しても public API は nongeneric のままである
- **WHEN** caller が default provider を custom provider へ上書きする
- **THEN** `ActorSystem` / `ActorRef` / typed system の public API 形状は変わらない
- **AND** provider 選択のために `ActorSystem<D>` のような公開 generic 型は導入されない

#### Scenario: default provider seeding は bootstrap 前に完了している
- **WHEN** `ActorSystemConfig::default()` から actor system を構築する初期化経路を確認する
- **THEN** `SystemState::build_from_config` が呼ばれる時点で default `ActorLockProvider` は config 内に存在する
- **AND** bootstrap 中に fallback 用 global state を参照しない

