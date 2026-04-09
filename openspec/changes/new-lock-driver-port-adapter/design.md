## Context

現状の `RuntimeMutex<T>` / `RuntimeRwLock<T>` は `SpinSyncMutex<T>` / `SpinSyncRwLock<T>` の alias であり、lock 実装差し替えの seam は存在しない。一方で前回案のように `RuntimeMutex<T, D>` として driver 型を公開側へ持ち出すと、`ActorRefSenderShared`、`MessageDispatcherShared`、`ExecutorShared`、`Mailbox`、`ActorCell`、最終的には `ActorSystem` まで `D` を伝播させるか、途中で不自然な erase を入れるかの二択になる。

今回 fix すべきなのは「`RuntimeMutex` が alias であること」そのものではなく、「driver selection の責務境界が API 形状に反映されていないこと」である。driver family は build-time ではなく実行時に決まる。実際、このプロジェクトでも `TickDriverConfig` は実行時オブジェクトを `ActorSystemConfig` から受け取って構築境界を固定している。

したがって新版 change では、`RuntimeMutex` の workspace-wide port/adaptor 化をやめ、actor system hot path だけを system-scoped `ActorLockProvider` で materialize する方針へ切り替える。

## Goals / Non-Goals

**Goals:**
- actor-core の再入 hot path で lock family を `ActorSystemConfig` 経由で選択できる
- `ActorSystem` / `ActorRef` / typed system の public API に driver generic parameter を漏らさない
- core は std adapter の concrete lock 型を知らない
- default spin / debug spin / std mutex の切り替えを actor system ごとに独立して行える
- same-thread 再入 tell を debug provider で観測できる

**Non-Goals:**
- `RuntimeMutex` / `RuntimeRwLock` を workspace-wide に port/adaptor 化すること
- 非 hot path の `RuntimeMutex` caller を一括移行すること
- 第 1 段階で `RwLock` 対称設計まで成立させること
- mailbox の lock-free 化や queue 内部二重ロック統合を同時に解くこと

## Decisions

### 1. driver selection は `ActorSystemConfig` に置く

lock family の選択点は `ActorSystemConfig` に置く。各 actor system は自分専用の `ActorLockProvider` を保持し、dispatcher bootstrap / configurator / mailbox wiring はその provider を使って hot path を構築する。

これにより、同一プロセス内で「system A は default spin」「system B は debug spin」のような共存が可能になる。global static や thread-local に「現在の driver」を置く案は、複数 system の共存と test isolation を壊すため採らない。

`ActorSystemConfig` と `ActorSystemSetup` には `with_lock_provider(...)` を追加し、内部 slot は `ArcShared<dyn ActorLockProvider>` とする。`ActorSystemConfig::default()` は `BuiltinSpinLockProvider` を seed し、明示 override がない限りこの既定 provider を使う。

この seeding は既存の default dispatcher seeding と同じ思想で扱う。すなわち「public config 型を増やさず default が必ず存在する」ではなく、「builder で override 可能だが、`default()` の時点で live な既定値が入っている」と定義する。

provider 契約・builtin 実装・`MailboxSharedSet` は `modules/actor-core/src/core/kernel/system/lock_provider/` 配下に集約する。`system/` 配下に置く理由は、provider が per-actor-system service であり、`system/extensions`、`system/coordinated_shutdown`、`system/state`、`system/guardian` と同じく "システムが起動から終了まで保持する横断的サービス" に分類されるためである。kernel root sibling として独立させると、機能ドメイン (`actor`, `dispatch`, `event` 等) と cross-cutting infra が同列に並んでしまい、概念分類の対称性が崩れる。`system/` 配下に置けば、dispatch / actor 側からの import 方向 (`dispatch::mailbox::base` → `system::state::SystemStateShared` 等) と整合し、新しい依存方向を作らずに済む。

ファイル構成は次のとおり (1file1type lint 準拠)。

```
kernel/system/lock_provider/
├── actor_lock_provider.rs        # trait ActorLockProvider
├── builtin_spin_lock_provider.rs # struct BuiltinSpinLockProvider
└── mailbox_shared_set.rs         # struct MailboxSharedSet
```

`kernel/system.rs` には `pub mod lock_provider;` を追加し、`pub use` で `ActorLockProvider` / `BuiltinSpinLockProvider` / `MailboxSharedSet` を `system` モジュール直下に再エクスポートする (caller 側のフルパスを短く保つため)。

`MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` の内部 lock backend は、それぞれの consumer module (`kernel/dispatch/dispatcher/`, `kernel/actor/actor_ref/`) に閉じた private trait (`DispatcherLocked` 等) として表現する。これらは公開 API ではなく、provider が `*Shared` を materialize する際の internal seam として機能する。

### 2. port surface は generic mutex ではなく `*Shared` factory にする

今回導入するのは `LockDriver<T>` のような汎用 generic port ではなく、actor system 用の object-safe provider である。provider は既存の `MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` を **直接 materialize する factory** として振る舞い、`RuntimeMutex<T, D>` のような型引数伝播を発生させない。

この判断により、前回案で破綻した「`D` をどこまで通すか」という論点を消せる。代わりに provider surface は actor system 専用になるが、今回必要なのはそこだけなので十分である。

最小スケッチは次で固定する。

```rust
pub trait ActorLockProvider: Send + Sync {
    fn create_message_dispatcher_shared(
        &self,
        dispatcher: Box<dyn MessageDispatcher>,
    ) -> MessageDispatcherShared;

    fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared;

    fn create_actor_ref_sender_shared(
        &self,
        sender: Box<dyn ActorRefSender>,
    ) -> ActorRefSenderShared;

    fn create_mailbox_shared_set(&self) -> MailboxSharedSet;
}
```

- provider は `*Shared` を直接返す。`*LockCell` のような中間 newtype は導入しない (`*Shared` 自身がすでに lock wrapper の責務を負っているため、cell を挟むのは責務二重化)
- `*Shared` の内部 lock backend は consumer module 側に閉じた private trait (`DispatcherLocked` / `ExecutorLocked` / `SenderLocked`) として表現し、`*Shared` が `ArcShared<dyn ...Locked>` を field に保持する形にする。trait と impl は public に出さない
- `MailboxSharedSet` は `Mailbox` が内部に持つ複数 lock を **同一 family で揃えて** 生成する bundle として残す。これは複数 lock を 1 引数で渡す意味があるため public 型として維持する
- provider の `dyn` dispatch は `*Shared` 構築時に 1 回だけ使い、message hot path は `*Shared` の既存メソッド経由で固定 dispatch される
- 第 1 段階では `RwLock` variant を provider surface に含めない
- `Executor` / `ActorRefSender` / `MessageDispatcher` は既存 actor-core trait 名をそのまま使う

つまり、今回の実装で field に保持されるのは `D` ではなく、provider が組み立て済みの `*Shared` (内部に provider 由来の opaque lock backend を抱える) である。

### 3. 第 1 段階は mutex 系 hot path に限定する

対象は `MessageDispatcherShared`、`ExecutorShared`、`ActorRefSenderShared`、`Mailbox` と、それらを wiring する `ActorCell` / `DispatcherSender` / configurator / bootstrap に限る。`RuntimeRwLock` を使う shared state や非 hot path caller は今回の対象外にする。

前回案では `Mutex` と `RwLock` を対称にしようとして scope が膨らんだ。今回は deadlock 観測に必要な mutex 系 hot path だけを first-class target にし、`RwLock` port 化は YAGNI と判断する。

### 4. provider 契約は actor-core に閉じ、Mailbox は bundle を受け取る

provider 契約は actor system 専用であるため actor-core に閉じる。今回の change では `utils-core` に一般化しない。これにより、非 hot path の `RuntimeMutex` alias と actor system 用 opaque provider surface を明確に分離できる。

Mailbox については個別 lock を 4 回 provider へ問い合わせるのではなく、`MailboxSharedSet` を 1 つ受け取る。理由は次のとおり。

- Mailbox の lock 群は常に同じ family で揃っている必要がある
- constructor の引数数を増やさずに済む
- same-thread 再入観測の対象を Mailbox 単位でまとめて固定できる

これにより crate 間の依存方向 (`actor-adaptor-std → actor-core → utils-core`) を守れる。前回破綻した「core の private alias で std adapter 型を切り替える」「`utils-adaptor-std` に actor system lock 契約を持ち込む」方向は、依存方向の逆転を招くため不採用とする。

### 5. dispatcher configurator を materialization boundary にする

`MessageDispatcherConfigurator` はもともと spawn / bootstrap 時に `MessageDispatcherShared` を materialize する境界である。新版ではここに `ActorLockProvider` を束縛する。

- `DefaultDispatcherConfigurator` / `BalancingDispatcherConfigurator` は、provider を束縛した shared instance を eager に構築して clone する
- `PinnedDispatcherConfigurator` は毎回新規 instance を返すが、使う provider family は configurator に束縛されたものとする
- `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` は mailbox を作るために provider handle を field として保持し、`create_mailbox` / `try_create_shared_mailbox` のたびに fresh な `MailboxSharedSet` を取得する
- 特に `BalancingDispatcher::create_mailbox` は shared queue 自体は既存どおり dispatcher 内部に保持しつつ、mailbox ごとの lock 群だけを provider から都度生成して `Mailbox::new_sharing(...)` へ渡す

message hot path で provider を resolve し直す案は、call-frequency contract と矛盾するため採らない。

### 6. debug provider の観測は panic に固定する

Phase 1 の debug provider は same-thread 再入を panic で報告する。error 返却や diagnostics event は今回採らない。

理由:

- deadlock 観測の主用途は test / explicit debug session である
- `same-thread 再入 tell が起きたら fail-fast` という contract のほうが test 化しやすい
- event stream や diagnostics registry まで巻き込むと scope が膨らむ

したがって、この change の観測 contract は「debug provider を明示 opt-in した actor system では、same-thread 再入 lock acquisition が panic する」で固定する。tests は `catch_unwind` でこれを観測する。

### 7. `RuntimeMutex` / `RuntimeRwLock` alias は残す

この change では `modules/utils-core/src/core/sync/runtime_lock_alias.rs` を全面再設計しない。既存 alias は非 hot path caller と test utility の既定面として残し、actor system hot path だけが provider 経由の opaque lock surface に移る。

これにより、170 を超える caller の一括巻き込みを避けられる。将来、workspace-wide な lock abstraction を本当に再設計したくなった場合は、別 change として扱う。

なお、`RuntimeMutex` / `RuntimeRwLock` という型名自体は **既存の utils-core alias** であり、命名規約 (`naming-conventions.md`) の `Runtime` 禁止サフィックスに該当する。本 change では Decision 7 のとおり既存 alias の rename はスコープ外とし、別 change で扱う。一方、本 change が新規導入する型・モジュール・capability・slot 名 には `Runtime` 接頭辞を一切使わない (`ActorLockProvider` / `BuiltinSpinLockProvider` / `with_lock_provider` / `actor-lock-provider` / `kernel/system/lock_provider/`)。

### 8. この change でいう bootstrap は `SystemState::build_from_config` と `ActorSystem::bootstrap` を指す

本 change で使う「bootstrap」は曖昧な総称ではなく、次の 2 段を指す。

- `SystemState::build_from_config(config)`
  - `ActorSystemConfig` に seed / override された `ActorLockProvider` を取り込み、dispatcher / mailbox / system state 側の構築に渡す段
- `ActorSystem::bootstrap(...)`
  - root guardian と spawn path が configurator 束縛済み provider family を使う段

この定義により、tasks 2.2 の対象は `ActorSystem::new_with_config_and` 配下の初期化経路と `SystemState::build_from_config` の 2 箇所であることを明確にする。

## Risks / Trade-offs

- [Risk] provider API が wrapper ごとの constructor 群になって広がる → Mitigation: 第 1 段階は hot path 4 型に限定し、非 hot path は既存 alias のまま維持する
- [Risk] `RuntimeMutex` alias と provider-based lock surface の二重系統がしばらく共存する → Mitigation: 役割を明記し、actor system hot path 以外へ provider を広げない
- [Risk] debug/std provider helper の配置境界が曖昧になる → Mitigation: default spin provider は actor-core だけで成立させ、actor system 専用の std helper は `actor-adaptor-std` の明示 API として提供する。`utils-adaptor-std` には `ActorLockProvider` 契約を持ち込まない
- [Risk] Mailbox だけ別経路の lock family を使うと再入検知が不完全になる → Mitigation: end-to-end test で `ActorRefSenderShared` / `MessageDispatcherShared` / `ExecutorShared` / `Mailbox` が同一 provider family で構築されることを確認する

## Migration Plan

1. `ActorSystemConfig` に `with_lock_provider(...)` slot を追加し、default config で `BuiltinSpinLockProvider` を seed する
2. actor-core に provider 契約と hot path 用 opaque lock surface を追加する
3. `SystemState::build_from_config` と `ActorSystem::bootstrap` を含む bootstrap wiring、dispatcher configurator、`ActorCell` wiring を provider ベースへ切り替える
4. `MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` / `Mailbox` を provider ベースで materialize する
5. `actor-adaptor-std` に debug/std provider helper を追加する
6. debug provider の same-thread 再入 panic test、default fallback test、複数 system 独立性 test を追加し、`openspec validate` で change 整合を確認する

## Open Questions

- なし。実装開始前に固定すべき骨格はこの design で決定済みとする。
