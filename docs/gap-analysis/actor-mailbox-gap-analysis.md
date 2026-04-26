# actor mailbox ギャップ分析

更新日: 2026-04-26

## 結論

Mailbox にスコープを絞って深さ優先で見ると、fraktor-rs の actor mailbox は「実行時の drain / system 優先 / dead letter 観測」というコア挙動はかなり Pekko 互換に近い。

一方で、弱いのは queue 実装そのものよりも **mailbox 選択契約** と **blocking bounded mailbox 契約** である。  
つまり、`actor` モジュール全体では高い parity に見えても、Mailbox だけを掘ると「実行コアは強いが、設定・選択・bounded semantics はまだ簡略化されている」という非対称さが見える。

## 比較スコープ定義

今回の mailbox 調査では次を parity 対象に含める。

| 領域 | fraktor-rs | Pekko |
|------|------------|-------|
| mailbox run loop / scheduling gate | `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala` |
| message queue family | `modules/actor-core/src/core/kernel/dispatch/mailbox/*message_queue*.rs` | `Mailbox.scala` の各 `MailboxType` / `MessageQueue` |
| mailbox registry / selection | `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs` | `Mailboxes.scala` |
| props / requirement / spawn-time validation | `modules/actor-core/src/core/kernel/actor/props/*.rs`, `modules/actor-core/src/core/kernel/system/base.rs` | `Mailboxes.scala`, `actor-typed/Props.scala` |
| dispatcher-mailbox binding | `modules/actor-core/src/core/kernel/dispatch/dispatcher/*.rs` | `Dispatcher.scala`, `Dispatchers.scala`, `BalancingDispatcher.scala` |

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
| requirement / capability gate | 中 | spawn 前 capability 検証はあるが、Pekko の queue type mapping までは持たない |
| mailbox selection / config 契約 | 中 | id registry はあるが、Pekko の `lookupByQueueType` や `bounded-capacity:` 相当はない |
| blocking bounded mailbox semantics | 低 | `pushTimeOut` ベースの bounded mailbox 契約は未実装 |
| BalancingDispatcher との mailbox 契約 | 中 | shared queue はあるが、Pekko の mailbox compatibility 契約は内部化されている |

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

`modules/actor-core/src/core/kernel/dispatch/mailbox.rs` には、Pekko mailbox 調査で期待する主要な queue family がまとまって存在する。

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

### 4. requirement / capability gate は部分対応

ここは「未実装」ではなく「Pekko より簡略化されている」が正しい。

fraktor-rs 側には:

- `MailboxRequirement` がある
- `Deque` / `ControlAware` / `BlockingFuture` の capability を宣言できる
- `ActorSystem::ensure_mailbox_requirements` が spawn 前に `requirement.ensure_supported(...)` を実行する
- `Props::with_stash_mailbox()` で stash に必要な deque requirement を付けられる

つまり、「この actor は deque-capable mailbox を必要とする」といった条件は検証できる。

ただし、Pekko と比べると次がない:

- `RequiresMessageQueue[T]` のような actor class ベースの queue type 宣言
- `mailbox.requirements` 設定から queue type を mailbox id へ引く mapping
- `lookupByQueueType(...)` による queue type 主導の mailbox 選択

したがって、**capability 検証はあるが、queue type 解決モデルは Pekko よりかなり薄い**。

### 5. mailbox selection / config 契約は部分対応

Pekko の `Mailboxes.getMailboxType(...)` はかなり多段である。

- `deploy.mailbox`
- dispatcher config 上の `mailbox-type`
- actor 側 requirement
- dispatcher 側 requirement
- default mailbox

さらに `bounded-capacity:` のような programmatic bounded mailbox helper もある。

これに対して fraktor-rs は次のように単純化されている。

- `Props::with_mailbox_id(...)` で registry lookup
- それ以外は `MailboxConfig`
- `Mailboxes::select_mailbox_type_from_config(...)` は
  - priority + stable-priority
  - control-aware requirement
  - deque requirement
  - default
  の順だけを見る

つまり、**選択ルールは理解しやすいが、Pekko の config-driven mailbox resolution とは同等ではない**。

ここで見えた具体差分:

- mailbox registry は単純な id → factory で、Pekko mailbox 側の alias chain 解決はない
- `bounded-capacity:` 相当の helper がない
- dispatcher config 側 mailbox requirement と actor 側 requirement の優先順位調停がない

### 6. blocking bounded mailbox semantics は弱い

今回の mailbox 深掘りで、もっともはっきりした非互換ポイントはここだった。

Pekko の bounded mailbox 群は `pushTimeOut` を持つ。

- `BoundedMailbox`
- `BoundedPriorityMailbox`
- `BoundedStablePriorityMailbox`
- `BoundedDequeBasedMailbox`
- `BoundedControlAwareMailbox`

そして `Mailboxes.lookupConfigurator(...)` は、非ゼロの `pushTimeOut` に対して warning を出す。

fraktor-rs にはこの系統の契約がない。bounded 系 queue はすべて次の overflow strategy モデルに寄っている。

- `DropNewest`
- `DropOldest`
- `Grow`

つまり fraktor-rs は「bounded queue が満杯のとき、待つか、一定時間 block するか」ではなく、「どのメッセージを捨てるか」で振る舞いを決めている。

この差は mailbox コアより大きい。  
**Pekko の bounded mailbox semantics をそのまま使う前提では、ここが最大のギャップである。**

### 7. control-aware mailbox は存在するが bounded semantics は変形されている

`BoundedControlAwareMessageQueue` は control queue と normal queue を分け、control を先に drain する点では Pekko 互換に近い。

ただし bounded overflow 時の扱いは Pekko と同型ではない。

- fraktor-rs は `DropOldest` 時に normal queue から優先的に evict する
- normal queue が空なら到着 control envelope を reject する
- これは「control message をなるべく落とさない」ための設計判断

Pekko は bounded control-aware mailbox を `pushTimeOut` 付き queue として扱うため、ここは **同じ目的に対する別設計** であり、完全互換ではない。

### 8. BalancingDispatcher と mailbox の契約は部分対応

Pekko の `BalancingDispatcherConfigurator` は mailbox requirement と mailbox type の互換を明示的に検証する。

- `MultipleConsumerSemantics` を要求する
- supplied mailbox が requirement を満たすか確認する

fraktor-rs の `BalancingDispatcher` は別アプローチで、dispatcher 内部に `SharedMessageQueue` を持ち、`try_create_shared_mailbox(...)` で sharing mailbox を返す。

これは runtime semantics としては妥当だが、次の差がある。

- custom mailbox type を balancing dispatcher に差し込む契約が露出していない
- compatibility check が mailbox type レベルではなく dispatcher 内部実装に吸収されている
- sharing mailbox は `MailboxPolicy::unbounded(None)` で構築されるため、Pekko の「互換 mailbox type を選ぶ」構図ではない

したがって、**負荷分散の実行セマンティクスはあるが、mailbox 契約としての Pekko parity は弱い**。

## 主要ギャップ

| ID | ギャップ | 重要度 | 内容 |
|----|----------|--------|------|
| MBX-H1 | blocking bounded mailbox 不在 | high | `pushTimeOut` 付き bounded mailbox 契約がなく、overflow strategy モデルへ置換されている |
| MBX-M1 | mailbox requirement 解決モデルの簡略化 | medium | capability 検証はあるが、`RequiresMessageQueue[T]` / `lookupByQueueType` / `mailbox.requirements` mapping がない |
| MBX-M2 | mailbox selection precedence の簡略化 | medium | Pekko の多段 config resolution ではなく、`mailbox_id` または `MailboxConfig` へ単純化されている |
| MBX-M3 | BalancingDispatcher mailbox compatibility 契約の内部化 | medium | multiple-consumer mailbox compatibility が dispatcher 内部実装に吸収され、mailbox type 契約としては露出しない |
| MBX-L1 | control-aware bounded overflow の差分 | low | control 優先保護のための独自 eviction ルールがあり、Pekko と完全同型ではない |

## まとめ

Mailbox だけを深く見ると、fraktor-rs の actor mailbox は「drain loop・system 優先・dead letter 観測・主要 queue family」という **実行時コア** は強い。

一方で、Pekko 互換性が弱いのは次の領域である。

- `pushTimeOut` を持つ bounded mailbox 契約
- queue type / requirement ベースの mailbox 選択
- BalancingDispatcher と mailbox type の compatibility contract

したがって、Mailbox スコープの結論は次の一文に尽きる。

**fraktor-rs の mailbox は runtime core parity は高いが、configuration-driven mailbox contract parity はまだ中程度である。**

もし次に mailbox parity をさらに詰めるなら、優先順位は以下が妥当である。

1. `pushTimeOut` 系 bounded mailbox を入れるか、意図的非互換として明文化する
2. `lookupByQueueType` 相当の requirement-driven mailbox 解決を追加する
3. BalancingDispatcher に mailbox compatibility contract を外部から見える形で持たせる
