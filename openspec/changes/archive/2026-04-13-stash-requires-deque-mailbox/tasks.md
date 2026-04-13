## 1. stash mailbox contract API の追加

- [x] 1.1 `modules/actor-core/src/core/kernel/actor/props/base.rs` に `Props::with_stash_mailbox()` を追加し、`MailboxRequirement::for_stash()` を適用する
- [x] 1.2 `modules/actor-core/src/core/typed/props.rs` に `TypedProps::with_stash_mailbox()` を追加し、underlying `Props` に stash requirement を委譲する
- [x] 1.3 `Props` / `TypedProps` の新 API 名と rustdoc を整理し、stash 用 contract であることを明記する
- [x] 1.4 `modules/actor-core/src/core/typed/dsl/behaviors.rs` の `Behaviors::with_stash(...)` rustdoc に「この helper は mailbox を設定しないため `TypedProps::with_stash_mailbox()` を併用すること」を明記する
- [x] 1.5 `./scripts/ci-check.sh ai dylint` を実行し、Section 1 の変更で lint エラーが増えていないことを確認する

## 2. stash / unstash の deterministic failure 化

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` に `pub(crate) fn user_queue_is_deque_capable(&self) -> bool` を追加する
- [x] 2.2 `modules/actor-core/src/core/kernel/actor/actor_cell.rs` の `stash_message_with_limit` で capability を確認し、non-deque mailbox なら stash buffer を触らずに recoverable error を返す
- [x] 2.3 `ActorCell::unstash_message` で message を `pop_front` する前に capability を確認し、non-deque mailbox なら stash buffer を触らずに reject する
- [x] 2.4 `ActorCell::unstash_messages` で `mem::take` する前に capability を確認し、non-deque mailbox なら stashed messages を触らずに reject する
- [x] 2.5 `ActorCell::unstash_messages_with_limit` で messages を取り出す前に capability を確認し、non-deque mailbox なら stash buffer を触らずに reject する
- [x] 2.6 `modules/actor-core/src/core/kernel/actor/actor_context.rs` に `STASH_REQUIRES_DEQUE_REASON` 定数と `is_stash_requires_deque_error(...)` 判定 helper を既存 stash error パターンに揃えて追加し、必要な re-export を行う
- [x] 2.7 `stash_message_with_limit` の capability check は `ActorContext` 側の `current_message` 前提解決の後で評価し、`unstash_*` は空 stash の場合に `Ok(0)` を先に返してから capability を評価する
- [x] 2.8 `./scripts/ci-check.sh ai dylint` を実行し、Section 2 の変更で lint エラーが増えていないことを確認する

## 3. stash caller の追従

- [x] 3.1 typed stash 関連 tests を `with_stash_mailbox()` 前提へ更新する
- [x] 3.2 classic stash 関連 tests を `with_stash_mailbox()` または同等の stash requirement 前提へ更新し、`unstash_messages_are_replayed_before_existing_mailbox_messages` も ordering を維持する形で修正する
- [x] 3.3 `showcases/std/stash/main.rs` を新 contract に合わせて更新する
- [x] 3.4 stash 利用箇所を検索し、意図的に non-deque failure を検証するもの以外は新 contract に揃える
- [x] 3.5 `./scripts/ci-check.sh ai dylint` を実行し、Section 3 の変更で lint エラーが増えていないことを確認する

## 4. contract 固定のテスト追加

- [x] 4.1 `Props::with_stash_mailbox()` が deque requirement を適用する test を追加する
- [x] 4.2 `TypedProps::with_stash_mailbox()` が deque requirement を適用する test を追加する
- [x] 4.3 non-deque mailbox で `stash_message_with_limit` が buffer を汚さず deterministic failure になる test を追加する
- [x] 4.4 non-deque mailbox で `unstash_message` / `unstash_messages` / `unstash_messages_with_limit` が stash を失わずに fail する test を追加する
- [x] 4.5 `bounded + stash` が `MailboxConfigError::BoundedWithDeque` で reject される test を typed/untyped 両方で固定し、両 helper の build path から同じ validation へ到達することを確認する
- [x] 4.6 deque mailbox requirement を満たした場合の ordering contract test が引き続き pass することを確認する
- [x] 4.7 `./scripts/ci-check.sh ai dylint` を実行し、Section 4 の変更で lint エラーが増えていないことを確認する

## 5. 検証

- [x] 5.1 `cargo check -p fraktor-actor-core-rs --lib` clean
- [x] 5.2 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass
- [x] 5.3 `cargo check -p fraktor-showcases-std` clean
- [x] 5.4 `openspec validate stash-requires-deque-mailbox --strict` valid
- [x] 5.5 最終的に `./scripts/ci-check.sh ai all` exit 0
