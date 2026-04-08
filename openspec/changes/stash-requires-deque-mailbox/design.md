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

#### Cross-cutting constraint: 現状は `bounded + deque` が成立しない

現行実装では `deque_mailbox_type_from_policy` が bounded policy を `MailboxConfigError::BoundedWithDeque` で reject する。したがって、**現状の mailbox factory のままでは `MailboxRequirement::for_stash()` と bounded mailbox を両立できない**。

この制約は Option A / B / C / E の mailbox-backed replay path に共通する。Option D は mailbox prepend を使わないため、この制約を直接は受けない。

したがって Phase 2 では次のどちらかを明示的に選ぶ必要がある:

- `bounded + stash` はサポート外として明示する
- `BoundedDequeMessageQueue` 相当を別 change で新設する

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
- 現状 `TypedProps::from_behavior_factory` は factory を spawn 時に 1 回だけ呼ぶ。Option A は **その 1 回の実行で得た `Behavior` を mailbox 構築前に inspect できる hook** を spawn 経路へ追加する必要がある。これは `ActorCell::create` と typed adapter 間の配線変更であり、単なる field 追加では済まない
- `bounded + stash` は現状の mailbox factory 制約により別途解決が必要

**実装コスト**: 中程度 (Behavior 構造変更 + spawn 経路変更 + 全 spawn test の確認)

**互換性**: 既存ユーザコードに対して非破壊。内部 spawn API の変更あり。

#### Option B: `Props` の mailbox requirement を明示的に使い、typed には薄い convenience を追加

```rust
impl<M> TypedProps<M> {
  pub fn with_stash_mailbox(self) -> Self {
    self.map_props(|props| props.with_mailbox_requirement(MailboxRequirement::for_stash()))
  }
}

// classic:
let props = Props::from_fn(factory)
  .with_mailbox_requirement(MailboxRequirement::for_stash());

// typed:
let props = TypedProps::from_behavior_factory(|| stash_behavior(0))
  .with_stash_mailbox();   // ← typed からの薄い convenience
```

`with_stash_mailbox()` は仮称であり、Phase 2 では `with_stash_support()` など user 意図をより直接表す名前へ見直す余地がある。

**Pros:**
- 既存の Behavior / Props の責務境界を保つ
- 明示的な opt-in なので silent なバグが減る
- 実装の中心は既存 `Props::with_mailbox_requirement(...)` の再利用で済む
- `TypedProps` 側の変更も薄い convenience method 1 つで足りる

**Cons:**
- ユーザが deque requirement を **必ず** 明示する必要がある。typed helper を呼び忘れると fallback/panic に落ちる
- 既存の `Behaviors::with_stash` を使うコードは破壊的 (要修正)
- `Behaviors::with_stash` を呼びつつ requirement を付け忘れる、という pitfall は依然残る
- 「stash を使う」という意図が API 上 2 箇所 (Behavior + Props) に分散する
- `bounded + stash` は現状の mailbox factory 制約により別途解決が必要

**実装コスト**: 低 (typed convenience method 追加 + docs/tests)

**互換性**: 既存 ユーザコードに対して破壊的 (build doesn't fail at compile time, but runtime fails)。

**成立条件 (重要):**
- Option B を採用する場合、Phase 2 では **silent fallback を許してはならない**。`prepend_via_drain_and_requeue` を削除したうえで、deque requirement を満たさない actor が stash/unstash を使った場合は **deterministic に失敗** しなければならない
- 失敗の発見タイミングは Phase 2 で詰めるが、少なくとも「helper 呼び忘れのまま production で fallback 経路に流れる」状態は不可
- したがって Option B は「explicit opt-in + fallback 削除 + deterministic validation」の 3 点セットで初めて spec を満たす
- practical な緩和策としては、少なくとも次のどちらかが必要:
  - `Behaviors::with_stash` 利用と mailbox requirement 未指定の組み合わせを検出する lint / static check
  - `prepend_via_drain_and_requeue` 削除後の deterministic validation (`panic` または spawn-time validation)

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
- `bounded + stash` 制約は残る

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
- mailbox に依存しないため、現状の `bounded + deque` 制約に縛られない

**Cons:**
- spec.md Requirement 3 の **ordering invariant** (`stashed messages are processed before existing pending mailbox messages`) を保たなければならない。この拘束を満たすには少なくとも次のいずれかが必要:
  - unstash 中に mailbox への新規 enqueue を block する
  - unstash 完了まで dispatcher loop を pause する
  - mailbox の pending と stashed を統合的に観測できる別の直列化境界を導入する
- 上記のどれを選んでも `user_queue_lock` 撤廃や dispatcher/runner 構造に追加コストが発生するため、Option D の実装難度は比較表以上に重い
- actor-core には現状 `Props::middleware()` に識別子を保持する API はあるが、typed/classic の runtime path でそれを解決して user callback に適用する mailbox-prepend middleware 実装は見当たらない。したがってここで言う middleware 懸念は **現行 runtime の破壊** ではなく、**将来 pipeline を追加したときの拡張性リスク** である
- ActorCell の `unstash_messages` シグネチャと caller (ActorCell::stash_unstash 経路) の大規模書き換え
- Pekko との互換性が崩れる可能性 (Pekko は mailbox レベルで unstash する)

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
- classic 側の既存 `Props::with_mailbox_requirement(...)` と typed 側の抜本解 (`Option D`) を分離して段階導入できる
- typed と classic の移行速度を分けられる。typed を先に改善しつつ、classic は既存 Props API に寄せたまま後追いで整理できる
- `bounded + stash` 制約の解き方を typed/classic で分離検討できるため、単一 option で両方を一度に着地させるより review を分割しやすい

**Cons:**
- 2 つの別々の機構を維持する必要がある (実装コストが最大)
- typed / classic の挙動が乖離する可能性
- レビューコストが最大
- classic では `bounded + stash` 制約が残る一方、typed では回避できるため、 capability の対称性が崩れる
- classic にも Option D を適用可能なら、この hybrid を独立 option として残す理由は弱くなる

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
| `bounded + stash` を新 queue なしで扱える | no | no | no | yes | typed=yes / classic=no |
| Pekko 互換 | yes | yes | yes | 要再検討 | typed=要再検討, classic=yes |
| Compile-time 強制 | no | no | no | no | no |
| 失敗検出時期 | n/a (auto) | lint または validation/panic | runtime panic | n/a | typed=n/a / classic=validation/panic |
| middleware 経路 | 不変 | 不変 | 不変 | 将来拡張時に再検討要 | typed=再検討 / classic=不変 |

### Decision 4: 推奨候補 (commit ではない)

**現時点での recommend は Option B** だが、user / team の判断を仰ぐ:

**理由**:
- 既存の `Behavior` / `Props` の責務境界を崩さない
- 既存の `Props::with_mailbox_requirement(...)` をそのまま活用できる
- 実装コストが最小に近く、YAGNI/`Less is more` に合う
- `prepend_via_drain_and_requeue` 削除という主目的を達成する
- Pekko 互換セマンティクスを維持できる
- middleware 経路に変化なし
- classic 側はすでに持っている API を活かせる

**懸念**:
- typed 側で explicit opt-in を忘れる pitfall は残る
- 既存 typed tests / examples の書き換えが必要
- compile-time 強制にはならない

**recommend の前提条件:**
- Option B を採用するなら、Phase 2 で helper 忘れを **silent に見逃さない** 緩和策を同時に入れる
- 最低ラインは「lint または deterministic validation」のどちらかであり、fallback を残したまま進めてはならない

**もし user が「既存 typed user code を壊したくない」と判断するなら → Option A**
- ただし `Behavior` の責務拡大と spawn 経路改修を受け入れる必要がある

**もし user が「mailbox prepend を完全に消したい」と判断するなら → Option D**
- ただし Pekko 互換セマンティクスの再検討が必要
- 実装コストは最大

### Decision 5: spec.md は option-agnostic に保つ

本 change Phase 1 の spec.md は、**どの option が選ばれても満たすべき不変条件** だけを記述する。具体的な実装詳細 (Behavior に field を追加するか、TypedProps に builder を追加するか) は記述しない。

不変条件:

1. stash 利用 actor は **必ず deque-capable mailbox** で実行される、または stash の replay が mailbox prepend を経由しない設計である
2. `Behaviors::with_stash` を使う既存 typed test (例: `typed_behaviors_stash_buffered_messages_across_transition`) は Phase 2 後も pass する
3. `cell.stash_message_with_limit` を使う既存 classic test (例: `unstash_messages_are_replayed_before_existing_mailbox_messages`) も pass する
4. stash → unstash の order semantic (stashed messages are processed before existing pending messages) は維持される

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
- recommend (Option B) を採用するか、別 option を選ぶか、修正を求めるかを判断
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
- **Q3**: Option D の middleware 経路問題は、Behavior layer から explicitly middleware pipeline を呼ぶ helper を提供すれば解消できるか?
  - 現状 actor-core には middleware identifier を `Props` に保持する API はあるが、runtime pipeline 実装自体は見当たらない
  - 将来 pipeline を追加する場合、dispatch 単位 (envelope vs typed message) をどう定義するかが論点になる
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
