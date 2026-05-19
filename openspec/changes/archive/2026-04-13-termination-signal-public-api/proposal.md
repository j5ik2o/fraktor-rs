## Why

現在の `when_terminated()` / `get_when_terminated()` は内部実装型 `ActorFutureShared<()>` をそのまま公開しており、利用者が `with_read(|f| f.is_ready())` と `thread::yield_now()` を組み合わせた busy wait に流れやすい。termination を観測する public API が内部 future primitive と直結しているため、同期 `main` と非同期 `main` の双方で安全な待機方法を定義しにくい。

## What Changes

- `ActorSystem::when_terminated()` と `TypedActorSystem::when_terminated()` が `ActorFutureShared<()>` ではなく termination 専用の公開型を返すようにする
- termination 専用公開型に、少なくとも「終了済み判定」と非同期文脈での安全な待機契約を与える
- 同期的な blocking wait が必要な場合は、core に `Blocker` port 契約を置き、std adapter 側にその実装を配置する
- `get_when_terminated()` も同じ公開型へ揃え、termination 観測 API を一本化する
- **BREAKING** `when_terminated()` / `get_when_terminated()` の戻り値型を変更する
- `showcases/std/getting_started/main.rs` を含む sample / test を新しい termination API に合わせて更新する
- `ActorFuture` / `ActorFutureShared` の ask 系・内部用途はこの change では直接変更しない

## Capabilities

### New Capabilities
- `termination-signal`: actor system termination を内部 future primitive から切り離し、安全な公開契約で観測できることを定義する

### Modified Capabilities
- `actor-runtime-safety`: termination 観測 API が busy wait を前提にしない安全な待機契約を持つように requirement を補強する
- `actor-std-adapter-surface`: std adapter が termination 用の `Blocker` 実装を提供できるよう公開面を拡張する

## Impact

- 影響コード:
  - `modules/actor/src/core/kernel/system/*`
  - `modules/actor/src/core/typed/system.rs`
  - `modules/actor/src/core/kernel/util/futures/*`
  - `modules/actor-adaptor/src/std/**`
  - `showcases/std/getting_started/main.rs`
- 影響 API:
  - `ActorSystem::when_terminated()`
  - `ActorSystem::run_until_terminated()`
  - `TypedActorSystem::when_terminated()`
  - `TypedActorSystem::get_when_terminated()`
- 非対象:
  - `ActorFuture` / `ActorFutureShared` の ask 用公開面の全面整理
  - `drain_ready_ask_futures()` の redesign
