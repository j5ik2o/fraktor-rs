## ADDED Requirements

### Requirement: async boundary は island ごとの actor 実行境界にならなければならない

stream materialization は、`async()` によって分割された各 island を独立した actor 実行単位として起動しなければならない（MUST）。複数 island graph を 1 つの actor が全 handle を順番に `drive()` する実装にしてはならない（MUST NOT）。

#### Scenario: 複数 island graph は island ごとの actor で drive される

- **GIVEN** `Source` / `Flow` / `Sink` graph に少なくとも 1 つの `async()` boundary がある
- **WHEN** `ActorMaterializer` が graph を materialize する
- **THEN** `IslandSplitter` が生成した island ごとに stream 実行 actor が起動される
- **AND** 各 actor は自分の island の `StreamHandleImpl` だけを所有する
- **AND** `StreamHandleImpl::drive()` は対象 island actor の mailbox 内で実行される

#### Scenario: 1 つの actor が複数 island handle を直列 drive しない

- **GIVEN** 3 つ以上の island に分割される graph
- **WHEN** materialized stream が tick を受ける
- **THEN** 1 つの actor が全 island handle を順番に `drive()` する経路は存在しない
- **AND** 各 island の drive はそれぞれの actor mailbox を通って実行される

### Requirement: async dispatcher は downstream island actor に反映されなければならない

`async_with_dispatcher(dispatcher_id)` は、その async boundary の downstream island actor を指定 dispatcher で起動しなければならない（MUST）。dispatcher id が actor system に登録されていない場合、materialization は default dispatcher へフォールバックしてはならない（MUST NOT）。

#### Scenario: async_with_dispatcher が downstream island の dispatcher を選ぶ

- **GIVEN** `Flow::new().map(...).async_with_dispatcher("stream-blocking")` を含む graph
- **AND** actor system に `"stream-blocking"` dispatcher が登録されている
- **WHEN** `ActorMaterializer` が graph を materialize する
- **THEN** async boundary の downstream island actor は `"stream-blocking"` dispatcher で起動される
- **AND** upstream island actor は dispatcher 指定がなければ default dispatcher で起動される

#### Scenario: 未登録 dispatcher は materialization failure になる

- **GIVEN** `Source::single(1).async_with_dispatcher("missing-dispatcher")` を含む graph
- **AND** actor system に `"missing-dispatcher"` dispatcher が登録されていない
- **WHEN** `ActorMaterializer` が graph を materialize する
- **THEN** materialization は失敗する
- **AND** default dispatcher への暗黙フォールバックは発生しない
- **AND** 失敗は `StreamError` として観測できる

### Requirement: materialized handle は graph 全体を代表しなければならない

複数 island graph の materialized handle は、先頭 island だけではなく graph 全体の lifecycle を代表しなければならない（MUST）。cancel / terminal state / snapshot は、materialized graph に属する全 island を対象にしなければならない（MUST）。

#### Scenario: 公開 handle は先頭 island だけを代表しない

- **GIVEN** 複数 island graph が materialize 済みである
- **WHEN** 利用者が `Materialized::handle()` から handle を取得する
- **THEN** その handle は materialized graph 全体を代表する
- **AND** 先頭 island の `StreamHandleImpl` だけを返す互換経路は存在しない

#### Scenario: cancel は全 island actor に伝播する

- **GIVEN** 複数 island graph が materialize 済みである
- **WHEN** 利用者が materialized handle を cancel する
- **THEN** materialized graph に属する全 island actor に cancel または shutdown command が送られる
- **AND** boundary は terminal state へ遷移し、pending 要素を無言で捨てない

#### Scenario: snapshot は island 単位の状態を観測できる

- **GIVEN** 複数 island graph が materialize 済みである
- **WHEN** materializer snapshot または test-only diagnostic が取得される
- **THEN** materialized graph に属する island 数を観測できる
- **AND** 各 island の dispatcher id または actor id を検証できる

### Requirement: materializer shutdown は island actors を決定的に停止しなければならない

`ActorMaterializer::shutdown()` は、materializer が起動した island actor と tick resource を停止しなければならない（MUST）。停止失敗を無言で握りつぶしてはならない（MUST NOT）。

#### Scenario: shutdown は全 island actor を停止する

- **GIVEN** 複数 materialized stream が存在し、それぞれが複数 island actor を持つ
- **WHEN** `ActorMaterializer::shutdown()` が呼ばれる
- **THEN** 全 island actor に shutdown command が送られる
- **AND** tick resource は cancel される
- **AND** shutdown が成功した後に island actor へ drive command は送られない

#### Scenario: shutdown failure は観測できる

- **GIVEN** island actor または tick resource の停止に失敗する状態がある
- **WHEN** `ActorMaterializer::shutdown()` が呼ばれる
- **THEN** 失敗は `StreamError` または actor error として観測できる
- **AND** best-effort コメントだけで失敗を黙殺しない

### Requirement: ActorSystem なし materializer は公開実行入口であってはならない

stream island actor 実行には ActorSystem が必須であるため、ActorSystem なし materializer helper は公開 runtime API として残してはならない（MUST NOT）。テスト補助として残す場合でも、`#[cfg(test)]` または `pub(crate)` に限定しなければならない（MUST）。

#### Scenario: ActorSystem なしで materialization は成功しない

- **GIVEN** ActorSystem を持たない materializer 構築経路が存在する
- **WHEN** runtime API から stream を materialize しようとする
- **THEN** materialization は成功しない
- **AND** ActorSystem なし直実行 API は公開されない

#### Scenario: テスト専用 helper は公開 API に現れない

- **GIVEN** ActorSystem なし materializer helper を unit test のために残す
- **WHEN** public API surface を確認する
- **THEN** helper は `#[cfg(test)]` または `pub(crate)` に限定される
- **AND** crate 利用者から runtime materializer として呼び出せない
