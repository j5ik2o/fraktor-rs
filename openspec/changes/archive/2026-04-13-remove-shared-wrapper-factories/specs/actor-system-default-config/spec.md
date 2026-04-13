## ADDED Requirements

### Requirement: actor system は shared runtime override seam を持たず builtin spin backend で shared runtime surface を seed しなければならない

actor system は、多数の型別 `*SharedFactory` や単一の lock factory seam を使った shared runtime override を持たず、builtin spin backend で default dispatcher と shared runtime surface を seed しなければならない（MUST）。`ActorSystemConfig` / `ActorSystemSetup` が `with_shared_factory(...)` や `with_lock_factory(...)` のような shared runtime override API を公開してはならない（MUST NOT）。

#### Scenario: default config は builtin spin backend で shared runtime surface を seed する
- **WHEN** caller が `ActorSystemConfig::default()` を使う
- **THEN** actor system は builtin spin backend を使って default dispatcher と shared runtime surface を構築する
- **AND** bootstrap path に shared runtime override 未設定の穴は存在しない

#### Scenario: actor system config は shared runtime override API を持たない
- **WHEN** actor system config の公開 API を確認する
- **THEN** caller は dispatcher / mailbox / extension などの runtime setting は設定できる
- **AND** shared runtime surface の backend を差し替える API は存在しない
- **AND** `with_shared_factory(...)` や `with_lock_factory(...)` は存在しない

#### Scenario: config API は nongeneric のまま保たれる
- **WHEN** actor system config の公開 API を確認する
- **THEN** `ActorSystem` / `ActorRef` / typed system の公開型に generic parameter は追加されない
- **AND** backend 選択のための公開 generic 型や runtime factory seam は導入されない
