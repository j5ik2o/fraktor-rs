## MODIFIED Requirements

### Requirement: actor system の shared factory 設定は default から spawn path まで一貫して反映されなければならない

actor system が選択した shared factory 設定は、default dispatcher の seed、spawn path、および shared runtime surface まで一貫して反映されなければならない（MUST）。`ActorSystemConfig` / `ActorSystemSetup` は単一 `ActorSharedFactory` に戻ってはならず（MUST NOT）、必要な個別 factory trait を bootstrap と spawn path へ確実に渡さなければならない（MUST）。

#### Scenario: default config は既定の shared factory 実装群を seed する
- **WHEN** caller が明示的な override なしで `ActorSystemConfig::default()` を使う
- **THEN** actor system は既定の shared factory 実装群を使って dispatcher と関連 shared runtime surface を seed する
- **AND** bootstrap path に shared factory 未設定の穴が存在しない

#### Scenario: custom override は bootstrap と spawn に貫通する
- **WHEN** caller が `ActorSystemConfig` または `ActorSystemSetup` に custom shared factory 実装を設定する
- **THEN** actor system の bootstrap / spawn path は必要な個別 factory Port をその custom 実装から取得して shared runtime surface を構築する
- **AND** 既定の shared factory 実装へ戻る silent fallback は存在しない

#### Scenario: config API は個別 Port 化後も公開 surface を nongeneric のまま保つ
- **WHEN** actor system config の公開 API を確認する
- **THEN** caller は shared factory override を設定できる
- **AND** `ActorSystem` / `ActorRef` / typed system の公開型に generic parameter は追加されない
- **AND** shared factory 差し替えのために `ActorSystem<T>` のような公開 generic 型は導入されない
