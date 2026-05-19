## ADDED Requirements

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
