## ADDED Requirements

### Requirement: actor system termination は TerminationSignal で観測されなければならない

actor system の termination 観測 API は、内部実装型 `ActorFutureShared<()>` ではなく `TerminationSignal` という専用公開契約を返さなければならない。利用者は内部 future primitive に依存せずに termination を観測できなければならない。

#### Scenario: classic actor system は termination signal を返す
- **WHEN** caller が `ActorSystem::when_terminated()` を呼ぶ
- **THEN** 戻り値は `TerminationSignal` である
- **AND** caller は `ActorFutureShared<()>` へ直接アクセスする必要がない

#### Scenario: typed actor system は同じ termination signal 契約を返す
- **WHEN** caller が `TypedActorSystem::when_terminated()` または `TypedActorSystem::get_when_terminated()` を呼ぶ
- **THEN** 戻り値は `TerminationSignal` である
- **AND** classic / typed で termination 観測契約が分岐しない

### Requirement: TerminationSignal は non-consuming な終了観測を提供しなければならない

`TerminationSignal` は、ある観測者が終了を待機または確認したあとでも、他の観測者が同じ終了状態を観測できる non-consuming contract を持たなければならない。

#### Scenario: 複数 clone が同じ終了状態を観測できる
- **WHEN** caller が同じ `TerminationSignal` を複数 clone して actor system の終了を待つ
- **THEN** いずれの clone も終了後に terminated 状態を観測できる
- **AND** 1つの clone が終了を観測しても他の clone から状態が消費されない

#### Scenario: 終了前後で terminated 状態が単調に変化する
- **WHEN** caller が actor system termination 前後で `TerminationSignal` の状態を確認する
- **THEN** termination 前は not terminated を返す
- **AND** termination 後は terminated を返し、その後 false に戻らない

### Requirement: TerminationSignal は core からプラットフォーム非依存に観測できなければならない

`TerminationSignal` は busy wait を利用者に強制してはならず、core からはプラットフォーム非依存な終了観測契約を提供しなければならない。`std` 依存の blocking wait は core へ持ち込んではならない。

#### Scenario: 非同期文脈では await できる
- **WHEN** caller が非同期 runtime 上で actor system termination を待つ
- **THEN** `TerminationSignal` は await 可能な契約を提供する
- **AND** caller は内部 listener 型や shared future primitive を直接扱わずに待機できる

#### Scenario: 同期向け blocking wait は core ではなく platform adapter に隔離される
- **WHEN** std 環境で同期的に actor system termination を待つ API が必要になる
- **THEN** その blocking wait は core の `Blocker` 契約と platform adapter 実装の組み合わせで提供される
- **AND** `core` の `TerminationSignal` 自体は `std::sync::Condvar` のような std 依存を持たない

### Requirement: blocking wait は `Blocker` port を通じて実現されなければならない

synchronous wait が必要な場合、termination の blocking wait は core に定義した `Blocker` port 契約を通じて実現されなければならない。`TerminationSignal` は特定 platform の待機機構を直接知ってはならない。

#### Scenario: TerminationSignal は `Blocker` 契約を使って同期待機できる
- **WHEN** caller が同期文脈で `TerminationSignal` の完了を待つ
- **THEN** caller は core の `Blocker` 契約を受け取る API を使って待機できる
- **AND** `TerminationSignal` 自体は std 実装型に依存しない
