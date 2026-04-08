## Context

### 現状の構造

```
Behavior layer                          Props layer                Mailbox layer
─────────────                           ───────────                ─────────────

Behaviors::with_stash(N, factory)       TypedProps::from_           ActorCell::create
  │                                      behavior_factory(...)        │
  ├─ Self::setup(|_| factory(            │                            ├─ Mailbox::new_from_config(
  │     StashBuffer::new(N)))            ├─ Props::from_fn(...)       │   &mailbox_config)
  │                                      │                            │
  └─ returns Behavior<M>                 └─ returns TypedProps<M>     └─ create_message_queue_from_config
                                            with default mailbox        │
                                            config (no deque req)       └─ default → UnboundedMessageQueue
                                                                                       (NOT deque!)

User code (later, inside actor handler):
  stash.stash(ctx)
    └─ ctx.stash_with_limit(capacity)
       └─ ActorCell::stash_message_with_limit
          └─ self.state.lock().stashed_messages.push_back(...)   ← stash storage は ActorCell, NOT mailbox

  // unstash 時:
  ctx.unstash_messages(N)
    └─ ActorCell::unstash_messages
       └─ self.mailbox().prepend_user_messages(&pending)         ← mailbox prepend を呼ぶ
          └─ Mailbox::prepend_user_messages
             ├─ self.user.as_deque() → None  (UnboundedMessageQueue は deque じゃない)
             └─ self.prepend_via_drain_and_requeue(...)          ← fallback 経路 (production!)
                ├─ outer user_queue_lock を取る
                ├─ self.user.dequeue() を全部 drain
                ├─ 新メッセージを enqueue
                └─ 既存メッセージを再 enqueue
```

### 問題の核心

`Behaviors::with_stash` は **Behavior layer のヘルパー** であり、Props/MailboxConfig には触れない。Behavior と Props は意図的に独立した責務 (Behavior = handler logic, Props = spawn configuration) を持っており、現状の API では Behavior から Props に「私は deque mailbox が必要」と伝える経路がない。

**この乖離は Pekko Akka との設計選択でもある**。Pekko の `Behaviors.withStash` はやはり Behavior のヘルパーで、Mailbox 設定とは独立だ。Pekko の場合は drain_and_requeue を bounded mailbox の `LinkedBlockingQueue` などが内部で吸収しており、外側の lock は持たない設計になっている。fraktor-rs は queue 実装が違うため、同じ問題が表面化している。

## Goals / Non-Goals

**Goals (本 change Phase 1):**

- stash 利用時の deque 要求伝達について **5 つの選択肢** を整理し、比較する
- それぞれの API impact / behavior change / 実装コスト / 互換性 / future change への影響を明確化する
- recommend 候補を提示するが **commit はしない**
- 最低限の不変条件 (どの option でも満たすべき contract) を spec.md に記述する
- user / team が次の判断 (どの option に進むか) をできる材料を揃える

**Non-Goals (本 change Phase 1):**

- **実装コードへの変更を一切しない**
- **test の追加・変更を一切しない**
- **option を 1 つに confirm しない** (合意は別の場で取る)
- **API signature を変更する具体的な diff を提示しない** (説明上のスケッチに留める)
- **Phase 2 の tasks / spec を本 change で書ききらない**

## Decisions

### Decision 1: 本 change は explore / proposal 型

通常の openspec change とは異なり、本 change は **実装プランの合意を得るための設計探索** が主目的である。proposal.md と design.md が成果物の中心で、tasks.md は Phase 1 (explore) のみを記述し、Phase 2 (合意後の実装) の tasks は意図的に **空** にする。

これは user の指示「ここは Claude が言うより難しく、Behaviors::with_stash が今は Props に触れないので、いきなり実装ではなく proposal/design 先行が安全です」に従うものである。

### Decision 2: 5 つの設計オプションを並列で比較する

以下のオプションを評価する。1 つに絞るのは本 change Phase 1 のスコープ外。

#### Option A: Behavior に `mailbox_requirement` field を追加し、spawn 時に Props に merge

```rust
pub struct Behavior<M> {
  // ... 既存 field ...
  mailbox_requirement: Option<MailboxRequirement>,   // ← 追加
}

impl Behaviors {
  pub fn with_stash<M, F>(capacity: usize, factory: F) -> Behavior<M> { /* ... */
    let mut behavior = Self::setup(/* ... */);
    behavior.mailbox_requirement = Some(MailboxRequirement::for_stash());
    behavior
  }
}

// ActorCell::create で Props.mailbox_config と Behavior.mailbox_requirement を merge:
let mailbox_config = props.mailbox_config().clone()
  .with_requirement(behavior.mailbox_requirement().unwrap_or_default());
```

**Pros:**
- ユーザは `Behaviors::with_stash(N, ...)` を呼ぶだけで自動的に deque mailbox が選ばれる
- 既存ユーザコードは無修正
- typed の API がそのまま使える

**Cons:**
- `Behavior<M>` の責務が広がる (handler logic だけでなく mailbox 要求も持つ)
- 「Behavior は spawn 時の設定に影響しない」という現状の不変条件が壊れる
- spawn 経路が Behavior を inspect する必要がある (Behavior factory が呼ばれた後で初めて分かる)
- BehaviorRunner や TypedActorAdapter の signature 変更が必要 (Behavior を inspect する hook を追加)
- spawn 時点で Behavior factory が一度呼ばれて、その後で実 actor 用にもう一度呼ばれる、という二重実行のリスクがある (これは現状 not yet で、Behavior factory は一度しか呼ばれない)

**実装コスト**: 中程度 (Behavior 構造変更 + spawn 経路変更 + 全 spawn test の確認)

**互換性**: 既存ユーザコードに対して非破壊。内部 spawn API の変更あり。

#### Option B: `TypedProps` (および classic `Props`) に `with_stash_capacity` builder を追加

```rust
impl<M> TypedProps<M> {
  pub fn with_stash_capacity(mut self, capacity: usize) -> Self {
    self.props = self.props.with_mailbox_requirement(MailboxRequirement::for_stash());
    // capacity は別途 attach (Behavior 側で持つか、Props metadata に持つ)
    self
  }
}

// ユーザは:
let props = TypedProps::from_behavior_factory(|| stash_behavior(0))
  .with_stash_capacity(8);   // ← 明示的に opt-in
```

**Pros:**
- 既存の Behavior / Props の責務境界を保つ
- 明示的な opt-in なので silent なバグが減る
- API 表面の変更が局所的 (TypedProps に builder method 1 つ追加)

**Cons:**
- ユーザが `with_stash_capacity` を **必ず** 呼ぶ必要がある。忘れると runtime で `prepend_via_drain_and_requeue` (将来削除予定) または panic
- 既存の `Behaviors::with_stash` を使うコードは破壊的 (要修正)
- `Behaviors::with_stash` を呼びつつ `with_stash_capacity` を呼び忘れる、という pitfall は依然残る
- 「stash を使う」という意図が API 上 2 箇所 (Behavior + Props) に分散する

**実装コスト**: 低 (builder method 追加 + ドキュメント)

**互換性**: 既存 ユーザコードに対して破壊的 (build doesn't fail at compile time, but runtime fails)。

#### Option C: Mailbox 側で runtime panic / diagnostics

```rust
// Mailbox::prepend_user_messages の冒頭で
if !self.user.as_deque().is_some() {
    panic!("stash/unstash requires deque-capable mailbox; configure Props::with_mailbox_requirement(MailboxRequirement::for_stash())");
}
```

**Pros:**
- 実装が最小 (1 行)
- 問題があれば即発見できる
- ユーザに対して明確なエラーメッセージ

**Cons:**
- panic は production で run-time 失敗。test では発見できるが、test カバレッジが低い production 環境ではその後 crash まで気づかない
- 「正しく動くべきコード」が configure 漏れで panic する設計は脆弱
- Option D / E のように prepend を不要にする抜本対応にはならない
- silent な性能劣化問題は解消される (panic で stop) が、production crash のリスクが新たに増える

**実装コスト**: 最小 (panic 1 行 + ドキュメント)

**互換性**: 既存ユーザコードに対して非常に破壊的 (panic = サービスダウン)。

#### Option D: stash の unstash を Behavior layer 内で完結 (mailbox prepend を使わない)

```
unstash_messages の経路を変更:

  ctx.unstash_messages(N)
    └─ ActorCell::unstash_messages
       └─ // 従来: mailbox.prepend_user_messages(&pending)
       └─ // 新設: BehaviorRunner レベルで stashed_messages を順次 invoke
          for message in pending {
            self.behavior_runner.invoke_directly(message)?;
          }
```

**Pros:**
- mailbox prepend が不要になる → `prepend_via_drain_and_requeue` も `prepend_via_deque` も両方撤廃可能
- `user_queue_lock` 撤廃の最大の障害が消える
- Behavior layer が stash の責務を完全に持つ (責務境界が明確)
- mailbox 種別 (deque か否か) と stash 機能が独立する

**Cons:**
- 「stashed messages は mailbox の existing pending よりも前に処理される」という Pekko 互換セマンティクスを保つために、unstash 中は新規 enqueue を block するか、unstash 完了後に mailbox を再 schedule するか、何らかの工夫が必要
- middleware 経路が変わる: 現状は mailbox prepend 後に通常の dispatcher loop が middleware を通すが、Option D では BehaviorRunner が直接呼ぶため middleware を bypass する可能性
- ActorCell の `unstash_messages` シグネチャと caller (ActorCell::stash_unstash 経路) の大規模書き換え
- Pekko との互換性が崩れる可能性 (Pekko は mailbox レベルで unstash する)
- stashed messages の **順序** と **新着 messages の interleaving** をどう定義するかの新たな設計判断が必要

**実装コスト**: 高 (ActorCell 経路 + Behavior layer の interpreter + middleware 統合 + tests)

**互換性**: API は維持されるが、observable behavior (順序 / middleware) が変わる可能性

#### Option E: Hybrid (typed = Option D, classic = Option B)

```
typed actor の stash:
  Behaviors::with_stash → Option D (Behavior layer で完結)
  → mailbox prepend 不要

classic actor の stash:
  Props::with_mailbox_requirement(MailboxRequirement::for_stash())
  → 必須化 (Option B)
  → mailbox prepend は使うが、deque mailbox 経由なので drain_and_requeue は不要
```

**Pros:**
- typed と classic それぞれに最適な方式を選べる
- typed users (新規 actor の主流) は zero-config で正しく動く
- classic users は明示的な opt-in (compile-time 保証ではないが pattern が単純)
- `prepend_via_drain_and_requeue` を完全削除可能

**Cons:**
- 2 つの別々の機構を維持する必要がある (実装コストが最大)
- typed / classic の挙動が乖離する可能性
- レビューコストが最大

**実装コスト**: 最大 (Option B + Option D の和)

**互換性**: typed 既存コードに対して非破壊、classic は要修正

### Decision 3: 比較表

| 項目 | Option A | Option B | Option C | Option D | Option E |
|---|---|---|---|---|---|
| 実装コスト | 中 | 低 | 最小 | 高 | 最大 |
| ユーザ既存コード破壊 | なし | 中 (要修正) | 大 (panic) | なし (typed) | なし (typed) / 中 (classic) |
| Behavior 責務拡大 | あり | なし | なし | あり | あり (typed) |
| `prepend_via_drain_and_requeue` 削除可能 | yes | yes | no | yes | yes |
| `user_queue_lock` 撤廃可能 | yes | yes | no | yes | yes |
| Pekko 互換 | yes | yes | yes | 要再検討 | typed=要再検討, classic=yes |
| Compile-time 強制 | no | no | no | no | no |
| 失敗検出時期 | spawn 時 (silent) | runtime panic | runtime panic | (該当なし) | runtime panic (classic) |
| middleware 経路 | 不変 | 不変 | 不変 | 変わる可能性 | 部分変化 |

### Decision 4: 推奨候補 (commit ではない)

**現時点での recommend は Option A** だが、user / team の判断を仰ぐ:

**理由**:
- 実装コストが妥当 (中)
- ユーザ既存コードを壊さない
- `prepend_via_drain_and_requeue` 削除という主目的を達成する
- Pekko 互換セマンティクスを維持できる
- middleware 経路に変化なし
- silent な性能劣化を解消する

**懸念**:
- Behavior の責務拡大は OO 的に微妙
- spawn 経路の改修コストが想定より大きい可能性 (BehaviorRunner / TypedActorAdapter の API 変更)

**もし user が「Behavior の責務を pure に保ちたい」と判断するなら → Option B**
- ただし silent なバグの pitfall を許容する必要がある
- ドキュメントで「stash を使う場合は必ず with_stash_capacity を呼ぶこと」を明示

**もし user が「mailbox prepend を完全に消したい」と判断するなら → Option D**
- ただし Pekko 互換セマンティクスの再検討が必要
- 実装コストは最大

### Decision 5: spec.md は option-agnostic に保つ

本 change Phase 1 の spec.md は、**どの option が選ばれても満たすべき不変条件** だけを記述する。具体的な実装詳細 (Behavior に field を追加するか、TypedProps に builder を追加するか) は記述しない。

不変条件:

1. stash 利用 actor は **必ず deque-capable mailbox** で実行される、または stash の replay が mailbox prepend を経由しない設計である
2. `Behaviors::with_stash` を使う既存 typed test (例: `typed_behaviors_stash_buffered_messages_across_transition`) は Phase 2 後も pass する
3. `cell.stash_message_with_limit` を使う既存 classic test (例: `unstash_messages_are_replayed_before_existing_mailbox_messages`) も pass する
4. stash → unstash の order semantic (stashed messages are processed before existing pending messages) は維持される、または明示的に変更が議論される

## Risks / Trade-offs

- **Risk**: 本 change Phase 1 では実装に進まないため、`remove-mailbox-outer-lock` 系の future change が **長期間 blocked** になる
  - **Mitigation**: Phase 1 は短期間で完了させる (1-2 営業日) 想定
- **Risk**: 5 オプションの比較が user に対して decision fatigue を生む
  - **Mitigation**: Decision 4 で recommend 候補を 1 つに絞っている。user は agree/disagree のみで進められる
- **Risk**: Phase 2 (合意後の実装) が当初想定より大規模になる (特に Option A / D)
  - **Mitigation**: Phase 2 開始時点で再度設計を refine。必要なら Phase 2 内で更に sub-phase に分ける
- **Trade-off**: 「Phase 2 を別 PR にする」ことで本 change が単独で merge 可能になるが、解決感が乏しい
  - **Acceptance**: 本 change Phase 1 の価値は「設計空間を整理し、合意を取るプロセスを走らせること」自体にある

## Migration Plan (Phase 1)

### Step 1: Phase 1 完了

- proposal.md / design.md / tasks.md / spec.md を整える
- `openspec validate stash-requires-deque-mailbox --strict` valid
- commit + push (PR は draft または explore tag で)

### Step 2: user / team レビュー

- 5 オプションを review
- recommend (Option A) を採用するか、別 option を選ぶか、修正を求めるかを判断
- 議論結果を design.md に追記

### Step 3: 合意後

- 本 change を archive または更新
- Phase 2 用の新規 change `stash-requires-deque-mailbox-impl-<option>` (仮称) を作成し、選ばれた option の実装を開始
- 本 change の Phase 2 セクションに合意した option へのポインタを追加

## Open Questions

- **Q1**: Behavior に mailbox_requirement field を追加することは、Behavior の責務として許容されるか?
  - 現状、Behavior は handler logic のみを保持する pure な abstraction
  - mailbox_requirement を持たせると「actor の実行環境要求」も Behavior の責務になる
  - これは Pekko の `Behaviors.setup` 内で `ActorContext.system` を介して mailbox を変更できる API と似ているが、ベスト practice として議論の余地がある
- **Q2**: stash unstash の **順序保証** は本当に必要か?
  - Pekko の現状: 「stashed messages are processed before existing pending messages」 は明示的な契約
  - fraktor-rs もこの契約を守る必要があるか? それとも relax して「いつかは processed される」程度でよいか?
  - 緩める場合は behavior change の document を追加
- **Q3**: Option D の middleware 経路問題は、Behavior layer から explicitly middleware pipeline を呼ぶ helper を提供すれば解消できるか?
  - 実装次第。middleware の dispatch 単位 (envelope vs typed message) を考慮する必要あり
- **Q4**: Phase 2 で実装した change は、本 change を update する形にするか、新規 change として作るか?
  - 現状の openspec workflow ではどちらも可能
  - recommend: 新規 change を作る (本 change は explore-only として archive)
- **Q5**: 本 change Phase 1 の spec.md は、option-agnostic な不変条件だけを書くべきか、それとも 5 オプションそれぞれの spec を併記するか?
  - 現状の draft は option-agnostic 1 つだけ
  - 5 オプション併記は spec が肥大化するため避ける
  - Phase 2 で選ばれた option に対応する spec が新規追加される
- **Q6**: `remove-mailbox-outer-lock` の future change は、本 change Phase 2 の **完了後** にしか実装できないが、待つ価値はあるか?
  - 価値あり: 本 change Phase 2 完了後は `prepend_via_drain_and_requeue` が確実に dead code になり、安全に削除できる
  - 待たない場合の代替: `remove-mailbox-outer-lock` を案 a2 (put_lock 限定化、最小変更) で進めるが、user は本提案を拒否済み (「YAGNI を悪用しない」)
  - したがって本 change Phase 2 完了を待つ
