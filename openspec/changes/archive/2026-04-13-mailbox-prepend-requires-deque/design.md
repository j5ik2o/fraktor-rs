## Context

現状の `Mailbox::prepend_user_messages(...)` は deque-capable queue なら `enqueue_first` を使うが、non-deque queue では `prepend_via_drain_and_requeue(...)` にフォールバックする。Phase III により stash 起点の production caller は deque mailbox requirement を満たすようになったため、現在の production caller は `ActorCell::unstash_message` / `unstash_messages` / `unstash_messages_with_limit` に実質限定されている。

その一方で、mailbox 実装自体にはなお drain-and-requeue fallback と、それを守るための `user_queue_lock` の compound-op 責務が残っている。Phase IV で outer lock reduction を提案するなら、まず `prepend_user_messages(...)` の契約を deque-only に固定し、fallback と lock 責務の一部を前もって切り離したほうが設計が明確になる。

この change は Phase III と Phase IV の間に置く Phase 3.5 として扱う。目的は prepend contract の hardening であり、lock 削減そのものではない。

## Goals / Non-Goals

**Goals:**
- generic prepend 呼び出しを廃止し、deque-capable queue を解決した caller だけが使える API に置き換える
- `prepend_via_drain_and_requeue(...)` を production path から外す
- current production caller が deque contract を満たしていることを固定する
- Phase IV が `user_queue_lock` の責務再評価に集中できる状態を作る

**Non-Goals:**
- `user_queue_lock` の削減や `put_lock` 限定化
- close correctness の再設計
- `Behaviors::with_stash` や `Props` の責務再変更
- bounded deque queue 実装の導入
- shared queue / BalancingDispatcher 経路の整理

## Decisions

### 1. Phase 3.5 は prepend contract hardening を独立 change として切り出す

この change は outer lock reduction の一部としてではなく、前提条件を固定する bridging change として扱う。

理由:
- Phase II は close correctness、Phase III は stash caller hardening だった
- ここで prepend contract まで固めると、Phase IV は lock 削減の設計だけに集中できる
- change を分けることで、semantic hardening と performance / concurrency 変更を混ぜずに済む

### 2. prepend 契約は型レベルで表現する

Phase 3.5 では runtime で non-deque prepend を拒否するのではなく、generic `Mailbox::prepend_user_messages(...)` を廃止し、deque-capable queue を事前に解決した caller だけが使える crate-private な prepend API に置き換える。

想定する形:
- `Mailbox::user_deque() -> Option<&dyn DequeMessageQueue>` または同等の read-only accessor を追加する
- `Mailbox::prepend_user_messages_deque(...)` のような deque 専用 API を追加する
- caller は `Option` 解決に失敗した時点で自前の contract violation として扱う

lock 責務は引き続き `Mailbox` 側に残す。`user_deque()` は caller の早期 reject 用 peek accessor に留め、actual prepend は `Mailbox::prepend_user_messages_deque(...)` の中で `user_queue_lock` を取得して行う。必要なら lock 取得後に `as_deque()` を再解決し、lock と queue capability の責務境界を崩さない。

したがって、Phase 3.5 の時点では caller が `&dyn DequeMessageQueue` をロック外へ引き回す設計は採らない。lock 段数や lock 配置の見直しは Phase IV のスコープである。

これにより prepend 契約違反は runtime の mailbox error ではなく、「API を呼ぶ前に解決すべき前提条件」になる。

採用理由:
- 型で表現できる契約を runtime error にしない
- public `SendError` も crate-private prepend error も増やさずに済む
- caller の事前 check 漏れに対して、API 形状自体が safety net になる

代替案:
- crate-private な prepend contract violation error を追加する
  - 型で表現できる前提を runtime error に落とす必要が薄いため採用しない
- panic / debug_assert にする
  - production contract が曖昧なまま残るので採用しない

### 3. `prepend_via_drain_and_requeue(...)` は削除対象とする

Phase III の完了後、current production caller に non-deque prepend は残っていない。したがって fallback は production path だけでなく、通常コード上も不要に近い。

この change では `prepend_via_drain_and_requeue(...)` を production path から切り離すのではなく、原則として削除する。関連 recovery/logging テストも新しい contract に合わせて整理する。

理由:
- lock 責務を温存する dead path を残さないため
- Phase IV で「まだ fallback があるから outer lock が必要」という議論を持ち込まないため

### 4. current production caller の前提を test で固定する

production caller inventory は現時点で `ActorCell::unstash_*` のみであり、Phase III ですでに deque contract を満たすよう harden 済みである。この前提を tests / examples / persistent props で固定する。

理由:
- Phase 3.5 の安全性は「残る caller が deque contract を満たす」ことに依存する
- inventory が変わったときに CI で気づけるようにしたい

## Risks / Trade-offs

- **[Risk]** hidden caller が non-deque prepend に依存している可能性がある
  - **Mitigation:** caller inventory を検索で固定し、workspace tests/examples を通す
- **[Risk]** fallback 削除後に Phase II 由来の prepend close-race coverage が落ちる
  - **Mitigation:** `base/tests.rs` の prepend 競合テストを deque queue 前提に更新し、close correctness coverage を維持する
- **[Risk]** deque-capable test fixture の新設が必要になる
  - **Mitigation:** `ScriptedDequeMessageQueue` 等の fixture 導入を task に含め、fallback 依存 fixture を先に置き換える
- **[Risk]** fallback 削除と outer lock reduction を一緒にやったほうが早く見える
  - **Mitigation:** semantic hardening と lock 削減を分離し、失敗時の切り戻し面を小さくする

## Migration Plan

1. `prepend_user_messages(...)` の caller inventory を再確認する
2. deque-capable test fixture と close-race coverage を先に移行する
3. deque 専用 prepend API を導入し、caller を移行する
4. `Mailbox::prepend_user_messages(...)` と non-deque fallback を除去する
5. 関連 tests / showcases / persistence caller が contract を満たすことを固定し、prepend close-race coverage を維持する
6. OpenSpec validate と full CI を通し、Phase IV の前提として扱える状態にする

## Open Questions

- 現時点ではなし

補足:
- 既存の `user_queue_is_deque_capable()` helper は `user_deque().is_some()` へ統合できるなら撤去する
- `prepend_via_deque` のロジックは、新しい deque 専用 prepend API の内部へ統合またはそのまま流用する
