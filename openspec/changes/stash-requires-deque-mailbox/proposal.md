## Why

`actor-core` の **stash 機構は production で deque-capable mailbox を要求していない**。これは設計の漏れであり、以下の不整合を生んでいる:

1. `MailboxRequirement::for_stash() = requires_deque()` という API は存在する (`mailbox_requirement.rs:25`)
2. `create_message_queue_from_config` は `requires_deque() = true` のときに `UnboundedDequeMessageQueue` を返す (`mailboxes.rs:57`)
3. しかし **`MailboxRequirement::for_stash()` の caller は test だけ** (`mailbox_requirement.rs` の grep で確認、production caller ゼロ)
4. `Behaviors::with_stash` (`typed/dsl/behaviors.rs:236`) は `Behavior::setup` を呼ぶだけで、Props/MailboxConfig には一切触らない
5. `TypedProps::from_behavior_factory` (`typed/props.rs:59`) も同様
6. 結果: typed actor が `Behaviors::with_stash` を使っても **default unbounded mailbox** (= `UnboundedMessageQueue`、非 deque) で spawn され、unstash 時の `prepend_user_messages` は **`prepend_via_drain_and_requeue` フォールバック経路** に流れる
7. 同じ問題は classic actor の `cell.stash_message_with_limit` 経路にも存在する (Props で deque 要求を明示しなければ非 deque mailbox で動作)

production が `prepend_via_drain_and_requeue` に依存している証拠:

- `typed_behaviors_stash_buffered_messages_across_transition` (`typed/tests.rs:230`): `TypedProps::from_behavior_factory(|| stash_behavior(0))` で default mailbox を使いつつ stash → unstash がパスしている
- `unstash_messages_are_replayed_before_existing_mailbox_messages` (`actor_cell/tests.rs:500`): `Props::from_fn(...)` で同じく default mailbox + unstash がパスしている
- これらが pass する唯一の理由は drain_and_requeue 経路が走っているから

### この設計が招く問題

- **`prepend_via_drain_and_requeue` を dead code として削除できない**: 撤回された `remove-mailbox-outer-lock` 提案 (2026-04-08) はこの fallback を dead code 扱いして削除しようとしたが、レビューで指摘されて撤回された
- **`user_queue_lock` (Mailbox の outer barrier lock) を撤廃できない**: `prepend_via_drain_and_requeue` が compound op の atomicity を outer lock に依存している。outer lock を外すには fallback を消す必要がある
- **`docs/plan/lock-strategy-analysis.md` の Phase II (二重ロック削減) が前進できない**: 上記 2 点の影響で、本来やりたい hot path のロック段数削減ができない
- **stash の deque 要求が型システムで保証されていない**: ユーザが `Behaviors::with_stash` を使ってもエラーは出ない。silent な性能劣化と暗黙の outer lock 依存だけが残る

### 設計が難しい理由

`Behaviors::with_stash` は **Behavior layer のヘルパー** であり、`Props.mailbox_config` への参照を持たない。Behavior と Props は意図的に独立した責務を持っており、現状の API では Behavior から Props.mailbox_config に対して deque 要求を伝達する自然な経路がない。

選択肢は複数あり、それぞれ異なる API impact / behavior 変化 / 実装コストを持つ。**proposal/design 先行で合意を取ってから実装に進む** 必要がある。本 change の責務は **設計空間を整理し、選択肢を提示すること** である。

## What Changes

本 change は **explore / proposal 型** であり、最終的な実装方針の合意を得るためのものである。実装タスクは **方針合意後に追加する** (本 proposal の段階では implementation tasks を含まない)。

### Phase 1 (本 change の責務): 設計空間の整理

- 後述の 5 つの選択肢 (Option A〜E) を design.md に詳述
- それぞれの API impact / behavior change / 実装コスト / リスクを比較表化
- recommend 候補を提示するが、commit はしない (user / team 判断を待つ)
- spec.md は **どの選択肢が選ばれても満たされるべき不変条件** だけを記述する (explore 段階の minimum spec)

### Phase 2 (合意後の別 commit / 別 PR): 選択された方針の実装

- 選ばれた option に対応する spec.md と tasks.md の追加
- 実装と test
- 本 proposal の更新 (合意済みの option を明記)

### 触らない範囲 (本 change の Phase 1 では)

- 実装コードへの変更は **一切なし**
- test の追加・変更も **一切なし**
- `stashed_messages` field や `stash_message_with_limit` の signature 変更も **一切なし**
- これらは Phase 2 で合意された option に応じて行う

## Capabilities

### Added Capabilities

- **`stash-mailbox-requirement`**: `actor-core` の stash 機構が **deque-capable mailbox を要求する** ことを設計の明示的な前提とする capability。Phase 1 では explore のみで、合意された設計が Phase 2 で実装される。

### Modified Capabilities

なし (本 change Phase 1 は新規 capability の追加 + design 整理のみ)。

## Impact

### 影響コード (Phase 1: 本 change)

なし。本 change Phase 1 はドキュメントのみ:

- `openspec/changes/stash-requires-deque-mailbox/proposal.md` (本ファイル)
- `openspec/changes/stash-requires-deque-mailbox/design.md` (5 オプションの詳細比較)
- `openspec/changes/stash-requires-deque-mailbox/tasks.md` (Phase 1 = explore tasks のみ)
- `openspec/changes/stash-requires-deque-mailbox/specs/stash-mailbox-requirement/spec.md` (option-agnostic な不変条件)

### 影響コード (Phase 2: 合意後の別 commit)

選ばれた option に依存:

- **Option A** (Behavior に requirements field 追加): `Behavior<M>` struct + 全 spawn 経路 + tests
- **Option B** (TypedProps explicit builder): `TypedProps::with_stash_capacity` 等の helper + ドキュメント + tests
- **Option C** (Mailbox runtime panic): 実装は最小だが behavior change 大きい
- **Option D** (stash unstash を Behavior layer 内で完結): `unstash_messages` 経路の大規模書き換え + Pekko 互換性検討
- **Option E** (Hybrid: typed = D, classic = B): 最大の作業量、最も柔軟

### 影響 API

Phase 1: なし
Phase 2: option 依存 (詳細は design.md)

### 観測可能な挙動の変化

Phase 1: なし
Phase 2: option 依存

### 後続 change への効果

合意された option が Phase 2 で実装されると、以下が解消される:

1. **`prepend_via_drain_and_requeue` を dead code として削除可能になる** (Option D 以外)
2. **`user_queue_lock` 撤廃 change (`remove-mailbox-outer-lock` の再提案) が安全に進められる**
3. **stash の deque 要求が型システムまたは API 契約で保証される**
4. **`docs/plan/lock-strategy-analysis.md` の Phase II が前進できる**
