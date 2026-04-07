## MODIFIED Requirements

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
