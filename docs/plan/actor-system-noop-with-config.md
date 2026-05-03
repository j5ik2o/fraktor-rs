# ActorSystem no-op guardian 生成 API 追加計画

## 概要

`GuardianActor` のような no-op user guardian を利用側で毎回定義しなくて済むように、`ActorSystem` に no-op guardian 付きで起動する public API を追加する。

既存の `new_empty` / `new_empty_with` は「guardian なし」の test helper なので、意味の衝突を避けて `empty_with_config` は追加しない。追加 API 名は `noop_with_config` に寄せる。

## 主要変更

- `actor-core` に内部 no-op actor 型を追加する。
  - 型名は `NoopGuardianActor`
  - 配置は `modules/actor-core/src/core/kernel/system/guardian/`
  - `receive` は常に `Ok(())`
  - public に露出させず、`ActorSystem` の生成 API からのみ使う
- `ActorSystem` に以下を追加する。
  - `pub fn noop_with_config(config: ActorSystemConfig) -> Result<Self, SpawnError>`
  - `pub fn noop_with_setup(setup: ActorSystemSetup) -> Result<Self, SpawnError>`
  - 実装は `Props::from_fn(NoopGuardianActor::new)` を作って既存の `create_with_config` に委譲する
- 既存テストの一部ボイラープレートを置き換える。
  - `modules/actor-adaptor-std/src/std/tick_driver/tests.rs` の `GuardianActor` を削除し、`ActorSystem::noop_with_config(config)` を使う
  - actor-core 側に no-op guardian 起動 API の契約テストを追加する

## Public API

```rust
impl ActorSystem {
  pub fn noop_with_config(config: ActorSystemConfig) -> Result<Self, SpawnError>;

  pub fn noop_with_setup(setup: ActorSystemSetup) -> Result<Self, SpawnError>;
}
```

`noop_with_config` は `/user` guardian を実際に起動する。したがって `actor_of`、`user_guardian_ref`、`terminate`、receptionist 初期化などは `create_with_config` と同じ契約で動く。

## Test Plan

- `ActorSystem::noop_with_config` が成功し、`state().has_root_started()` が true になること
- `user_guardian_ref()` が取得でき、パスが `/user/...` 配下になること
- `actor_of` で user guardian 配下に actor を spawn できること
- `terminate()` が成功すること
- `ActorSystemConfig::default()` のように tick driver がない設定では、既存どおり `SpawnError::SystemBuildError` になること
- `rtk cargo test -p fraktor-actor-core-rs actor_system_noop_with_config`
- `rtk cargo test -p fraktor-actor-adaptor-std-rs tick_driver`
- ソース編集後の最終確認として `./scripts/ci-check.sh ai all`

## 前提

- `empty` は既存の「guardian なし」語彙として維持し、今回の用途には使わない。
- no-op guardian 型は API として公開しない。利用者には `ActorSystem::noop_with_config` と `ActorSystem::noop_with_setup` だけを提供する。
- 後方互換は不要だが、既存 API の削除は今回の目的外なので行わない。
