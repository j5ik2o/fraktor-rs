## Why

前回の `lock-driver-port-adapter` は、`RuntimeMutex` に driver 型 `D` を持ち込んだことで破綻した。lock driver の concrete 実装は `ActorSystemConfig` / dispatcher configurator / std adapter helper が実行時に選ぶものであり、`D` を型として外へ出すと `ActorCell` から `ActorSystem` まで型引数が伝播し、public API への abstraction leak と core/std 境界の崩壊を招く。

必要なのは `RuntimeMutex` 全面 port 化ではない。actor-core の再入 hot path だけが、runtime に選ばれた lock family で構築され、同時に `ActorSystem` / `ActorRef` / typed system の public API は nongeneric のまま保たれる境界である。

## What Changes

- `RuntimeMutex<T, D>` / `RuntimeRwLock<T, D>` のような workspace-wide generic driver 方針をやめる
- actor runtime 専用の `runtime lock provider` 境界を導入し、lock family の選択を `ActorSystemConfig` / dispatcher configurator / bootstrap の実行時構成へ移す
- `MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` / `Mailbox` は、型引数 `D` を公開せず、選択済み provider から構築される opaque な hot path surface に寄せる
- `RuntimeMutex` / `RuntimeRwLock` / `NoStdMutex` は既存 caller 向けの既定 alias として維持し、非 hot path の workspace-wide 置換はこの change に含めない
- 第 1 段階は mutex 系 hot path に限定し、`RwLock` の対称 port 化は要求しない
- `utils-adaptor-std` は debug 用 / std 用の runtime lock provider helper を提供し、actor-core test から same-thread 再入検知を有効化できるようにする
- `Mailbox::new` / `Mailbox::new_sharing` は `MailboxLockSet` を追加引数で受け取り、lock 構築境界を provider ベースへ移す
- `mailbox-runnable-drain` の `run` / enqueue / cleanup の意味論は変更しない。変わるのは Mailbox constructor surface だけである

## Capabilities

### New Capabilities
- `actor-runtime-lock-provider`: actor runtime hot path を、public generic を漏らさない system-scoped runtime lock provider で構築できる

### Modified Capabilities
- `dispatcher-trait-provider-abstraction`: dispatcher configurator が runtime lock provider を束縛して `MessageDispatcherShared` を materialize する
- `actor-system-default-config`: actor system default config が既定 provider を seed し、明示 override を受け付ける
- `mailbox-runnable-drain`: `Mailbox::new` / `Mailbox::new_sharing` が `MailboxLockSet` を受け取り、drain semantics を維持したまま constructor surface を更新する

## Impact

- 影響コード:
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs`
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_setup.rs`
  - `modules/actor-core/src/core/kernel/system/base.rs`
  - `modules/actor-core/src/core/kernel/system/state/system_state.rs`
  - `modules/actor-core/src/core/kernel/runtime_lock_provider/`（新設）
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/*configurator*.rs`
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs`
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs`
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender.rs`
  - `modules/actor-core/src/core/kernel/actor/actor_ref/actor_ref_sender_shared.rs`
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
  - `modules/utils-adaptor-std/src/`
- 影響 API:
  - `ActorSystemConfig` の runtime lock provider 設定面
  - `ActorSystemSetup` の runtime lock provider 設定面
  - dispatcher configurator の構築面
  - public `ActorSystem` / `ActorRef` / typed system は nongeneric のまま維持
- 検証:
  - debug provider を使った same-thread 再入 tell の panic 観測 test
  - std provider override と default spin fallback の構成 test
  - 同一プロセス内の複数 actor system が独立した provider family を使えることの test
