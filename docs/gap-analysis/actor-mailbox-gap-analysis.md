# actor mailbox ギャップ分析

更新日: 2026-06-19

## 結論

Mailbox にスコープを絞って深さ優先で見ると、fraktor-rs の actor mailbox は「実行時の drain / system 優先 / dead letter 観測」というコア挙動はかなり Pekko 互換に近い。

2026-06-19 時点では、`RequiresMessageQueue` / `ProducesMessageQueue` / `MessageQueueSemantics`, `Mailboxes::lookup_by_queue_type`, `MailboxSelection`, `BalancingDispatcherFactory::new_checked` により、Pekko の queue type 宣言・selection precedence・balancing compatibility contract も公開契約として到達可能になった。さらに `MailboxPolicy::with_push_timeout` により、Pekko `pushTimeOut` に相当する finite/zero timeout bounded enqueue 契約も公開 API として到達可能になった。

Mailbox スコープで残る差分は、`push_timeout` を使わない bounded control-aware mailbox の overflow ルールが control 保護のために Pekko と完全同型ではない点に限られる。

## 比較スコープ定義

今回の mailbox 調査では次を parity 対象に含める。

| 領域 | fraktor-rs | Pekko |
|------|------------|-------|
| mailbox run loop / scheduling gate | `modules/actor-core-kernel/src/dispatch/mailbox/base.rs` | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala` |
| message queue family | `modules/actor-core-kernel/src/dispatch/mailbox/*message_queue*.rs` | `Mailbox.scala` の各 `MailboxType` / `MessageQueue` |
| mailbox registry / selection | `modules/actor-core-kernel/src/dispatch/mailbox/mailboxes.rs` | `Mailboxes.scala` |
| props / requirement / spawn-time validation | `modules/actor-core-kernel/src/actor/props/*.rs`, `modules/actor-core-kernel/src/system/*.rs` | `Mailboxes.scala`, `actor-typed/Props.scala` |
| dispatcher-mailbox binding | `modules/actor-core-kernel/src/dispatch/dispatcher/*.rs` | `Dispatcher.scala`, `Dispatchers.scala`, `BalancingDispatcher.scala` |

今回のスコープから外すもの:

| 除外項目 | 理由 |
|----------|------|
| remote `AddressTerminated` 連携 | mailbox 単体ではなく remote / actor 境界の課題 |
| supervision restart stash の全体設計 | mailbox requirement との接点だけ見る。再起動戦略全体は別スコープ |
| testkit 専用 mailbox | runtime mailbox parity ではない |

## サマリー

| 観点 | 評価 | 要点 |
|------|------|------|
| run loop / scheduling | 高 | system message 優先、suspend 中の scheduling gate、throughput deadline、cleanup は強い |
| queue family surface | 高 | bounded / unbounded / deque / priority / stable-priority / control-aware が揃う |
| overflow / dead letter observability | 高 | reject / evict を dead letter に観測可能化している |
| requirement / capability gate | 高 | actor 側 `RequiresMessageQueue` と mailbox 側 `ProducesMessageQueue` / `MessageQueueSemantics` で capability contract を表現できる |
| mailbox selection / config 契約 | 高 | explicit id → dispatcher id → actor requirement → dispatcher requirement → default の precedence と `lookup_by_queue_type` がある |
| blocking bounded mailbox semantics | 高 | `MailboxPolicy::with_push_timeout` と mailbox clock 連携で finite/zero `pushTimeOut` 相当の bounded enqueue 契約を実装済み |
| BalancingDispatcher との mailbox 契約 | 高 | `MultipleConsumerSemantics` 相当の requirement と `BalancingDispatcherFactory::new_checked` / `is_mailbox_compatible` がある |

## 詳細評価

### 1. run loop / scheduling gate は高い互換性

fraktor-rs の `Mailbox::run` は、コメントと実装の両方で Pekko `Mailbox.run()` を明示的に追っている。  
`process_all_system_messages` を先に回し、その後に `process_mailbox` を進める順序は Pekko と一致している。

根拠:

- fraktor-rs: `Mailbox::run`, `process_all_system_messages`, `process_mailbox`
- Pekko: `Mailbox.run`, `processAllSystemMessages`, `processMailbox`

特に互換性が高い点:

- system message を user message より常に先に drain する
- suspend 中は user dequeue を止めるが、system message は通す
- throughput deadline を mailbox 側で判定する
- close 後 cleanup で system queue を dead letters に必ず流す

この領域は「Mailbox のコア挙動」としては strong で、深掘りしても大きな欠落は見えなかった。

### 2. queue family surface もかなり揃っている

`modules/actor-core-kernel/src/dispatch/mailbox.rs` には、Pekko mailbox 調査で期待する主要な queue family がまとまって存在する。

確認できた主な型:

- `BoundedMessageQueue` / `UnboundedMessageQueue`
- `BoundedDequeMessageQueue` / `UnboundedDequeMessageQueue`
- `BoundedPriorityMessageQueue` / `UnboundedPriorityMessageQueue`
- `BoundedStablePriorityMessageQueue` / `UnboundedStablePriorityMessageQueue`
- `BoundedControlAwareMessageQueue` / `UnboundedControlAwareMessageQueue`

単に名前があるだけでなく、`Mailboxes::select_mailbox_type_from_config` で priority / stable-priority / control-aware / deque を切り替える経路も存在する。

### 3. overflow と dead letter 観測は強い

fraktor-rs は bounded queue overflow を「黙って捨てる」のではなく、mailbox 層で dead letter に記録する設計になっている。  
`Mailbox::enqueue_envelope` は `EnqueueOutcome::Evicted` と `Rejected` を成功扱いしつつ、`DeadLetterReason::MailboxFull` で観測可能化している。

これは Pekko の「enqueue 呼び出し側には成功に見えるが、損失は dead letters へ流す」という運用感にかなり近い。

強い点:

- `DropNewest` を `Rejected` として扱い、dead letter 記録する
- `DropOldest` を `Evicted` として扱い、押し出された envelope を dead letter 記録する
- priority queue / stable-priority queue でも overflow 観測を維持する

### 4. requirement / capability gate は高い互換性

ここは「未実装」ではなく、Pekko の marker trait model を Rust の明示的な trait / value contract に置き換えた対応済み領域である。

fraktor-rs 側には:

- `MailboxRequirement` がある
- `Deque` / `ControlAware` / `BlockingFuture` の capability を宣言できる
- `MultipleConsumerSemantics` 相当の capability を宣言できる
- actor type が `RequiresMessageQueue` で要求 queue semantics を宣言できる
- mailbox factory が `produced_queue_semantics()` / `ProducesMessageQueue` で提供 semantics を宣言できる
- `ActorSystem::ensure_mailbox_requirements` が spawn 前に `requirement.ensure_supported(...)` を実行する
- `Props::with_stash_mailbox()` で stash に必要な deque requirement を付けられる
- `Props::with_required_message_queue::<A>()` / `Props::from_required_fn::<_, A>(...)` で actor type の requirement を props へ反映できる

つまり、「この actor は deque-capable mailbox を必要とする」といった条件は検証できる。

Pekko と完全同型ではない点は残る:

- Scala/JVM の class hierarchy に基づく自動 reflection はない
- HOCON の `mailbox.requirements` をそのまま読む configurator はない

したがって、**capability 検証と queue type contract は実装済みだが、HOCON / reflection 駆動の configurator は Rust の別設計として対象外**である。

### 5. mailbox selection / config 契約は高い互換性

Pekko の `Mailboxes.getMailboxType(...)` はかなり多段である。

- `deploy.mailbox`
- dispatcher config 上の `mailbox-type`
- actor 側 requirement
- dispatcher 側 requirement
- default mailbox

さらに `bounded-capacity:` のような programmatic bounded mailbox helper もある。

fraktor-rs は `MailboxSelection` と `Mailboxes::select(...)` で、Pekko 型の precedence を明示的に表現する。

- explicit mailbox id
- dispatcher mailbox id
- actor 側 requirement の `lookup_by_queue_type(...)`
- dispatcher 側 requirement の `lookup_by_queue_type(...)`
- default mailbox

つまり、**選択 precedence と queue type lookup は公開 API として対応済み**である。

残る差分は HOCON 固有の configurator 形状である。

- `bounded-capacity:` 相当の helper がない
- Pekko mailbox 側の alias chain / fallback chain を HOCON として読む実装はない

これは `actor-gap-analysis.md` の固定スコープでは JVM/HOCON configurator を n/a としているため、残ギャップには数えない。

### 6. blocking bounded mailbox semantics は対応済み

今回の mailbox 深掘りで残っていた最大ギャップはここだったが、2026-06-19 時点で解消済みである。

Pekko の bounded mailbox 群は `pushTimeOut` を持つ。

- `BoundedMailbox`
- `BoundedPriorityMailbox`
- `BoundedStablePriorityMailbox`
- `BoundedDequeBasedMailbox`
- `BoundedControlAwareMailbox`

そして `Mailboxes.lookupConfigurator(...)` は、非ゼロの `pushTimeOut` に対して warning を出す。

fraktor-rs では `MailboxPolicy::with_push_timeout(Some(Duration))` で timeout-aware bounded enqueue を選択できる。mailbox runtime は `MessageQueue::enqueue_with_mailbox_clock(...)` / `DequeMessageQueue::enqueue_first_with_mailbox_clock(...)` に mailbox clock を渡し、bounded queue family は満杯時に空きが出るまで deadline まで再試行する。deadline 到達時は到着 envelope を `Rejected` / `SendError::Timeout` として返し、mailbox 層が dead letters へ観測可能化する。

対象 queue family:

- `BoundedMessageQueue`
- `BoundedPriorityMessageQueue`
- `BoundedStablePriorityMessageQueue`
- `BoundedDequeMessageQueue`（front insertion を含む）
- `BoundedControlAwareMessageQueue`

`push_timeout` を指定しない場合は従来どおり `DropNewest` / `DropOldest` / `Grow` の overflow strategy が有効になる。したがって、非互換だった「bounded queue が満杯のとき一定時間待つ」契約は opt-in compatibility path として到達可能になった。

Rust 側の API は `core::time::Duration` を受けるため、Pekko の HOCON / JVM configurator 形状そのものではなく、finite/zero timeout の queue contract を Rust API として表現する。

### 7. control-aware mailbox は存在するが bounded semantics は変形されている

`BoundedControlAwareMessageQueue` は control queue と normal queue を分け、control を先に drain する点では Pekko 互換に近い。

ただし bounded overflow 時の扱いは Pekko と同型ではない。

- fraktor-rs は `DropOldest` 時に normal queue から優先的に evict する
- normal queue が空なら到着 control envelope を reject する
- これは「control message をなるべく落とさない」ための設計判断

`push_timeout` を指定した場合は bounded control-aware mailbox も timeout-aware enqueue になり、満杯時に既存 envelope を evict しない。`push_timeout` を指定しない `DropOldest` path では、control 保護のために normal queue を優先 evict する独自ルールが残る。ここは **同じ目的に対する別設計** であり、完全互換ではない。

### 8. BalancingDispatcher と mailbox の契約は高い互換性

Pekko の `BalancingDispatcherConfigurator` は mailbox requirement と mailbox type の互換を明示的に検証する。

- `MultipleConsumerSemantics` を要求する
- supplied mailbox が requirement を満たすか確認する

fraktor-rs は `MailboxRequirement::requires_multiple_consumer()` と `MessageQueueSemantics::satisfies(...)` でこの contract を表現し、`BalancingDispatcherFactory::new_checked(...)` / `is_mailbox_compatible(...)` で public に検証できる。

runtime 側では引き続き dispatcher 内部に `SharedMessageQueue` を持ち、`try_create_shared_mailbox(...)` で sharing mailbox を返す。これは実行方式の違いであり、互換性チェックの公開契約は存在する。

残る差分は、Pekko の「設定ファイルから互換 mailbox type を選ぶ」構図ではなく、Rust API で factory semantics を直接検証する点である。

したがって、**負荷分散の実行セマンティクスと mailbox compatibility contract はどちらも対応済み**である。

## 主要ギャップ

| ID | ギャップ | 重要度 | 内容 |
|----|----------|--------|------|
| MBX-L1 | control-aware bounded overflow の差分 | low | control 優先保護のための独自 eviction ルールがあり、Pekko と完全同型ではない |

## まとめ

Mailbox だけを深く見ると、fraktor-rs の actor mailbox は「drain loop・system 優先・dead letter 観測・主要 queue family」という **実行時コア** は強い。2026-06-19 時点では、queue type / requirement ベースの mailbox 選択、BalancingDispatcher の mailbox compatibility contract、`pushTimeOut` 相当の timeout-aware bounded enqueue contract も公開 API として揃っている。

一方で、Pekko 互換性に差分が残るのは次の領域である。

- `push_timeout` を使わない bounded control-aware mailbox の独自 overflow rule

したがって、Mailbox スコープの結論は次の一文に尽きる。

**fraktor-rs の mailbox は runtime core、selection contract、blocking bounded semantics の parity が高く、残差分は control-aware bounded overflow の低優先度な設計差に限られる。**

もし次に mailbox parity をさらに詰めるなら、優先順位は以下が妥当である。

1. `push_timeout` を指定しない bounded control-aware overflow rule を Pekko と完全同型に寄せる必要があるか再評価する
