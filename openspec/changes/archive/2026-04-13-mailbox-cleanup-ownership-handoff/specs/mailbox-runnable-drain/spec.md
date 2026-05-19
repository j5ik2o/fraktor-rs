## MODIFIED Requirements

### Requirement: mailbox は terminal cleanup ownership を finalizer として保持できる

mailbox は、自身の `run()` に通常 drain ループだけでなく terminal cleanup ownership handoff を統合しなければならない（MUST）。close request が立った後、mailbox の user lane を最終的に片付ける主体は finalizer として **exactly once** で選出されなければならない（MUST）。

finalizer は次のどちらかである:

- idle mailbox を close した caller
- close request 中の mailbox を走行中の runner

#### Scenario: running mailbox は close request 後に finalizer へ遷移できる
- **GIVEN** mailbox が `run()` 中である
- **WHEN** 別経路から mailbox に close request が立つ
- **THEN** running mailbox は current in-flight work を超えて通常の user dequeue を無制限に継続しない
- **AND** `run()` 終了前に finalizer へ遷移できる

#### Scenario: finalizer は queue cleanup を exactly once で実行する
- **WHEN** idle detach caller と running runner が同じ mailbox の close request に関与する
- **THEN** queue drain / dead-letter / `clean_up()` を実行する主体は 1 回だけ選ばれる
- **AND** 二重 cleanup は起こらない

#### Scenario: close request 後の queued user messages は通常 delivery されない
- **GIVEN** mailbox が既に user message を queue に保持している
- **AND** close request が立った時点で runner が 1 件の user message を既に in-flight にしている可能性がある
- **WHEN** finalizer が残 queue を処理する
- **THEN** in-flight 済み以外の queued user messages は actor へ通常 delivery されない
- **AND** cleanup policy に従って dead-letter 化または skip される

