# actor-core-remote-router-serialization Specification

## Purpose
TBD - created by archiving change remote-phase2-medium-gaps. Update Purpose after archive.

## Requirements
### Requirement: RemoteRouterConfig は wire-safe consistent-hashing pool を serialize できる

`actor-core-kernel` の built-in misc serializer は、`RemoteRouterConfig` が wire-safe consistent-hashing pool を含む場合に encode/decode できなければならない (MUST)。wire-safe consistent-hashing pool は、`ConsistentHashableEnvelope` の明示 `hash_key` を routing key として使う built-in mapper に限定されなければならない (MUST)。

任意クロージャの `hash_key_mapper` を持つ `ConsistentHashingPool` は serialize してはならない (MUST NOT)。その場合、serializer は `NotSerializable` を返さなければならない (MUST)。

#### Scenario: envelope-hash-key consistent-hashing pool は round-trip できる

- **GIVEN** `RemoteRouterConfig` が envelope-hash-key consistent-hashing pool と remote nodes を持つ
- **WHEN** `MiscMessageSerializer` で encode し、`RORRC` manifest で decode する
- **THEN** decoded value は `RemoteRouterConfig` である
- **AND** local pool は `RemoteRouterPool::ConsistentHashing` として復元される
- **AND** `nr_of_instances`、router dispatcher、remote nodes は元の値を保つ

#### Scenario: arbitrary closure mapper は NotSerializable のまま

- **GIVEN** `ConsistentHashingPool::new(n, |message| ...)` で作られた pool を含む `RemoteRouterConfig`
- **WHEN** `MiscMessageSerializer::to_binary` を呼ぶ
- **THEN** `Err(SerializationError::NotSerializable(_))` が返る
- **AND** serializer は closure を型名、debug 文字列、固定値などへ変換して wire に載せない

#### Scenario: unknown consistent-hashing mapper tag を拒否する

- **GIVEN** encoded `RemoteRouterConfig` の consistent-hashing mapper tag が未定義値に書き換えられている
- **WHEN** `MiscMessageSerializer` が decode する
- **THEN** `Err(SerializationError::InvalidFormat)` が返る

#### Scenario: envelope-hash-key mapper は explicit hash key を優先する

- **GIVEN** envelope-hash-key consistent-hashing pool から router を作成する
- **AND** message payload が `ConsistentHashableEnvelope` である
- **WHEN** router が routee を選択する
- **THEN** routing key は envelope の `hash_key()` から導出される
- **AND** arbitrary closure mapper は要求されない
