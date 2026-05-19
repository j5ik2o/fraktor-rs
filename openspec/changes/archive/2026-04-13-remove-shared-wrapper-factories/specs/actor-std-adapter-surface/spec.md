## ADDED Requirements

### Requirement: std adapter 公開面は actor runtime の shared factory / lock factory override surface を公開してはならない

std adapter の system 公開面は、actor runtime の shared wrapper / shared state backend を差し替える surface を公開してはならない（MUST NOT）。`shared_factory` module、`StdActorSharedFactory`、`DebugActorSharedFactory`、およびそれらの rename 版となる lock factory concrete 型を公開面に残してはならない（MUST NOT）。

#### Scenario: std 公開面から shared factory module が除外される
- **WHEN** 利用者が std adapter の system 公開面を確認する
- **THEN** `std::system::shared_factory` module は存在しない
- **AND** `StdActorSharedFactory` と `DebugActorSharedFactory` は利用できない
- **AND** `StdActorLockFactory` や `DebugActorLockFactory` のような代替公開型も存在しない

#### Scenario: std adapter 利用コードは default builtin spin 構成を前提にする
- **WHEN** std adapter を使う example、test、または利用コードが actor system を構築する
- **THEN** それらは `with_shared_factory(...)` や `with_lock_factory(...)` を使わない
- **AND** actor runtime の shared wrapper / shared state backend 切替を前提にしない
