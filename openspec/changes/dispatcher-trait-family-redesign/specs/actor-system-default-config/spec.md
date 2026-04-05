## MODIFIED Requirements

### Requirement: actor system の default dispatcher は public config 型なしで解決できる

actor system の default dispatcher 要件は、`DispatcherConfig::default()` のような public config 型ではなく、system が解決可能な default dispatcher entry の存在として定義されなければならない。

#### Scenario: dispatcher 未指定の actor は default entry から解決される
- **WHEN** actor が dispatcher を明示せずに起動する
- **THEN** bootstrap は system に登録された default dispatcher entry を解決する
- **AND** caller が `DispatcherConfig` を構築しなくても default dispatcher が適用される

#### Scenario: system default config は caller の明示登録なしで default entry を提供する
- **WHEN** caller が dispatcher registry を追加設定せずに actor system default config を使う
- **THEN** system は reserved default dispatcher entry を保持して起動できる
- **AND** caller に default dispatcher の明示登録を要求しない

#### Scenario: default dispatcher 要件は provider registry ベースで説明される
- **WHEN** actor system の default dispatcher 要件を確認する
- **THEN** その要件は default dispatcher entry と provider registry の存在で説明される
- **AND** `DispatcherConfig::default()` の存在を前提にしない

#### Scenario: feature 差は runtime fallback ではなく提供面の差として現れる
- **WHEN** `tokio-executor` feature の有無で std adapter の dispatcher 公開面を確認する
- **THEN** `DefaultDispatcher` の提供有無は feature で明示される
- **AND** thread backend への暗黙 fallback で default 要件を満たしたことにしない
