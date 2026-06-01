# プロダクト概要

fraktor-rs は、Apache Pekko と Proto.Actor のセマンティクスを Rust に移植する、仕様駆動の actor runtime です。
移植性の高い `no_std` core と host runtime 向け adaptor を分離し、組み込み・サーバー双方で同じ契約を再利用できることを重視します。

## 中核機能

- Actor system の基本構成: actor ref、supervision、death watch、routing、dispatcher、mailbox、event stream を runtime の中核として扱う。
- Typed actor facade: untyped kernel の上に typed API、DSL、receptionist、pub-sub、delivery、typed system surface を構築する。
- Persistence: event sourcing、journal、snapshot、persistent actor/FSM、durable state、typed persistence effector を actor runtime の契約として提供する。
- Remote / cluster: address、association、transport port、watcher、wire format、identity lookup、placement、activation/passivation、topology、downing を port-first に表現する。
- Streams: stream DSL、graph shape、stage/materialization contract、queue、kill switch、supervision、actor integration を `no_std` core と std adaptor に分ける。

## 想定ユースケース

- Pekko / Akka 系 actor model を Rust で実装・検証したい runtime 開発。
- `no_std` 制約下でも成立する actor / stream / persistence の状態機械や契約設計。
- Tokio や TCP などの host 固有実装を core から切り離した、port-and-adapter 型の runtime 実装。
- 参照実装との差分を OpenSpec と gap analysis で管理しながら段階的に runtime parity を上げる開発。

## 提供価値

- `*-core` と `*-adaptor-*` の分離により、移植可能な contract と host binding を混在させない。
- 参照実装由来のセマンティクスを、そのまま写経するのではなく Rust の型・所有権・`no_std` 制約に合わせて再設計する。
- OpenSpec、repository rules、custom dylint、CI を組み合わせ、設計意図と実装境界を機械的に守る。
- 実行可能な showcase を API usage の実例として維持し、ドキュメントだけでなく実行できるふるまいで runtime surface を確認する。

---
_網羅的な機能一覧ではなく、判断に使う目的とパターンだけを記録する。ここに従う新機能追加では steering 更新が不要になる粒度を保つ。_
