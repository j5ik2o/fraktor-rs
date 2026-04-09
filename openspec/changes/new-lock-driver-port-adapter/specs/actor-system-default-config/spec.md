## ADDED Requirements

### Requirement: actor system default config は default runtime lock provider を seed し、明示 override を許可する

`ActorSystemConfig::default()` は、caller が追加設定しなくても actor runtime hot path を構築できる default runtime lock provider を seed しなければならない（MUST）。同時に caller は system ごとに明示 override できなければならない（MUST）。

#### Scenario: default config は追加設定なしで provider を持つ
- **WHEN** caller が runtime lock provider を明示設定せずに `ActorSystemConfig::default()` を使う
- **THEN** actor system は default runtime lock provider を使って起動できる
- **AND** caller に std adapter helper や build-time generic parameter を要求しない

#### Scenario: caller は system ごとに provider を上書きできる
- **WHEN** caller が `ActorSystemConfig` に対して custom runtime lock provider を設定して起動する
- **THEN** その system の hot path は custom provider で構築される
- **AND** 他の actor system の default provider 構成には影響しない

#### Scenario: setup facade からも同じ override ができる
- **WHEN** caller が `ActorSystemSetup` を使って custom runtime lock provider を設定する
- **THEN** 最終的な `ActorSystemConfig` にはその provider が保持される
- **AND** `ActorSystemConfig` へ直接設定した場合と同じ意味論で扱われる

#### Scenario: override しても public API は nongeneric のままである
- **WHEN** caller が default provider を custom provider へ上書きする
- **THEN** `ActorSystem` / `ActorRef` / typed system の public API 形状は変わらない
- **AND** provider 選択のために `ActorSystem<D>` のような公開 generic 型は導入されない

#### Scenario: default provider seeding は bootstrap 前に完了している
- **WHEN** `ActorSystemConfig::default()` から actor system を構築する初期化経路を確認する
- **THEN** `SystemState::build_from_config` が呼ばれる時点で default runtime lock provider は config 内に存在する
- **AND** bootstrap 中に fallback 用 global state を参照しない
