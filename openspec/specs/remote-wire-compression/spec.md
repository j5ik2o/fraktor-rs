# remote-wire-compression Specification

## Purpose
TBD - created by archiving change remote-wire-compression. Update Purpose after archive.
## Requirements
### Requirement: Compression table state を保持する

`remote-core` は actor ref と serializer manifest の compression table state を no_std compatible な型として提供する SHALL。table state は kind ごとに independent であり、actor ref table は canonical actor path、manifest table は serializer manifest 文字列を扱う。table state は literal 値ごとの hit count、entry id、generation、advertisement 済み状態、ack 済み状態を保持しなければならない（MUST）。

`RemoteCompressionConfig` の対象 kind の max が `None` の場合、その kind の compression table は disabled として扱い、hit count、advertisement、compressed reference encode を行ってはならない（MUST NOT）。

この disabled state は local outbound compression の無効化を表し、peer から届く inbound advertisement の適用と inbound table reference の復元を拒否してはならない（MUST NOT）。

#### Scenario: 繰り返し literal は hit count を更新する

- **WHEN** actor ref table に同じ canonical actor path を複数回 observe する
- **THEN** table はその literal の hit count を増加させる
- **AND** duplicate entry id を作らない

#### Scenario: disabled kind は hit を収集しない

- **GIVEN** actor-ref compression max が `None` である
- **WHEN** actor ref table に canonical actor path を observe する
- **THEN** table は hit count と advertisement candidate を保持しない

#### Scenario: disabled kind は inbound advertisement を適用できる

- **GIVEN** actor-ref compression max が `None` である
- **WHEN** peer から actor ref table advertisement を受信する
- **THEN** table は advertised entry ids と literal values を inbound resolution 用に保持する
- **AND** outbound encode は actor ref literal のまま維持される

#### Scenario: table max は advertised entries を制限する

- **GIVEN** manifest compression max が `2` である
- **AND** 3 つ以上の manifest literal が observe されている
- **WHEN** table が advertisement generation を作成する
- **THEN** advertisement entry 数は 2 を超えない
- **AND** advertised entries は deterministic な順序で選ばれる

### Requirement: Advertisement と acknowledgement の意味論

compression table は peer ごと、kind ごとに advertisement generation を作成し、peer から acknowledgement を受信した generation だけを compressed reference encode に使う SHALL。advertisement は entry id と literal value を含み、acknowledgement は kind と generation を echo しなければならない（MUST）。

#### Scenario: advertisement は pending generation を作成する

- **WHEN** table が advertisement generation を作成する
- **THEN** table は advertised entries と generation を pending ack として保持する
- **AND** ack を受信するまではその generation の entries を compressed reference encode に使わない

#### Scenario: acknowledgement は compressed references を有効化する

- **GIVEN** table が generation `7` の advertisement を送信済みである
- **WHEN** peer から kind と generation `7` の acknowledgement を受信する
- **THEN** table は generation `7` の entries を ack 済みとして扱う
- **AND** outbound encode は該当 literal を table reference として表現できる

#### Scenario: stale acknowledgement は無視する

- **GIVEN** latest pending generation が `8` である
- **WHEN** peer から generation `7` の acknowledgement を受信する
- **THEN** table state は generation `8` の pending ack を維持する
- **AND** stale generation の entries を新たに ack 済みにしない

### Requirement: Literal fallback と invalid reference handling

compression table は ack 済み reference が存在しない literal を outbound encode するとき、literal 表現へ fallback する SHALL。inbound compressed reference が local inbound table に存在しない場合は observable protocol failure として扱い、別の literal へ silently fallback してはならない（MUST NOT）。

#### Scenario: unknown outbound literal は literal のまま送る

- **GIVEN** actor ref table に ack 済み entry が存在しない
- **WHEN** outbound envelope の recipient path を encode する
- **THEN** recipient path は literal として encode される

#### Scenario: acked outbound literal は reference を使う

- **GIVEN** actor ref table に `/user/a` の ack 済み entry id `3` が存在する
- **WHEN** outbound envelope の recipient path `/user/a` を encode する
- **THEN** recipient path は table reference id `3` として encode される

#### Scenario: inbound unknown reference は fail closed になる

- **GIVEN** inbound actor ref table に entry id `9` が存在しない
- **WHEN** inbound envelope が recipient path の table reference id `9` を含む
- **THEN** transport decode または compression resolution は observable error を返す
- **AND** envelope は actor delivery へ進まない

