## ADDED Requirements

### Requirement: Provider input は Grain runtime が使う前に正規化される

Cluster provider は、Grain runtime の identity / placement logic が観測する前に、discovery / seed / lifecycle / explicit membership operation を cluster topology input へ変換する SHALL。Cluster core は、Grain runtime topology invalidation を適用するとき provider-specific discovery details で分岐しない MUST。

#### Scenario: topology update は provider-neutral である

- **WHEN** provider が `ClusterEvent::TopologyUpdated` を publish する
- **THEN** cluster core は provider type に依存しない topology input として update を扱う
- **AND** Grain runtime invalidation rules は、その update に含まれる authorities に対して動作する

#### Scenario: discovery details は core placement の外側に残る

- **WHEN** topology input が static configuration / remoting lifecycle / AWS ECS task discovery に由来する
- **THEN** identity and placement logic は変換後の authority set を使う
- **AND** identity and placement logic は元の discovery backend を inspect しない

### Requirement: Local / static provider は境界づけられた membership behavior を公開する

Local / static provider は provider boundary で membership behavior を定義する SHALL。Local provider は membership を変更する explicit join / leave / down operation に対して topology update を publish する MUST。Static provider は start 時に configured static topology だけを publish し、discovery を実行しない MUST。

#### Scenario: local explicit join は joined topology を publish する

- **WHEN** local provider が non-member authority に対する explicit join を受け取る
- **THEN** その authority を `joined` に含めた topology update を publish する
- **AND** その authority は provider の current member set に入る

#### Scenario: local explicit leave / down は left topology を publish する

- **WHEN** local provider が current member authority に対する explicit leave または down を受け取る
- **THEN** その authority を `left` に含めた topology update を publish する
- **AND** その authority は provider の current member set から削除される

#### Scenario: static provider は start 時に configured topology を publish する

- **WHEN** static provider が configured topology を持って start する
- **THEN** その topology を cluster topology update として publish する
- **AND** discovery subscription や polling task を start しない

### Requirement: Core-defined port は std adapter が実装する

Cluster core は provider port と lifecycle policy を定義する SHALL。一方で std adapter は std-specific lifecycle / discovery source 向けにその port を実装する SHALL。Std adapter は Grain runtime policy を所有しない MUST。Remoting lifecycle subscription は返された subscription lifetime によって制御され、strong reference で local provider を生存させ続けない MUST。AWS ECS discovery polling は AWS ECS provider の start / shutdown lifecycle が所有する MUST。

#### Scenario: remoting adapter は provider port input を供給する

- **WHEN** std adapter が local provider port implementation を remoting lifecycle events に subscribe する
- **THEN** subscription が保持されている間、connected events は local provider join input になり得る
- **AND** subscription を drop すると、その adapter は追加の topology input 生成を止める

#### Scenario: remoting subscription は provider を強参照で保持しない

- **WHEN** caller が local provider への strong handle をすべて drop する
- **THEN** remoting subscription は provider を生存させ続けない MUST
- **AND** その後の remoting lifecycle events は、その adapter によって無視される

#### Scenario: AWS ECS polling は provider-owned に留まる

- **WHEN** AWS ECS provider が member または client として start する
- **THEN** ECS polling lifecycle を所有し、discovered running tasks から topology updates を publish する
- **AND** cluster core は結果として出る topology update events だけを観測する

### Requirement: Downing はこの capability では input boundary に留める

Provider boundary は explicit down または provider-observed departure が member departure input を生成できるようにする SHALL。この capability は failure observation policy、Split Brain Resolver behavior、reachability matrix semantics、rebalance、remembered entity recovery を定義しない MUST。

#### Scenario: explicit down は departure input へ変換される

- **WHEN** provider が current member authority に対する explicit down を受理する
- **THEN** その authority に対する member departure topology input を生成する
- **AND** Grain runtime invalidation は stale activation と PID cache removal を扱う

#### Scenario: downing decision policy は provider boundary の外側にある

- **WHEN** failure detector または downing strategy が suspected member を観測する
- **THEN** この capability は member を down すべきかを決定しない
- **AND** 将来の downing decision model は別 capability で仕様化する
