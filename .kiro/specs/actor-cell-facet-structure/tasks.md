# 実装計画

- [x] 1. 基盤
- [x] 1.1 ActorCell facet module の骨格を作る
  - root ActorCell は生成、identity/accessor に集中する形へ移行できる状態にする
  - `actor_cell.rs` は child module を持たない leaf module として維持し、`actor.rs` の `pub use actor_cell::ActorCell` を変更しない
  - private sibling facet module は `actor.rs` から `mod actor_cell_dispatch;` のように宣言し、親 actor module の public surface が増えていないことを確認する
  - root または sibling facet から必要な helper は `pub(super)` に限定し、facet 分割のための新しい `pub(crate)` / `pub use` を追加しない
  - root ActorCell の最小 creation/accessor 回帰が green になる
  - _Requirements:_ 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2
  - _Boundary:_ ActorCell Core
  - _Depends:_ none

- [x] 2. コア facet 分割
- [x] 2.1 Dispatch Facet を分離する
  - user message、system message、mailbox pressure の delivery bridge を dispatch 責務として切り出す
  - `ActorCellInvoker` は dispatch facet 内に閉じ、root `ActorCell::create` からは `pub(super)` factory/helper 経由で `MessageInvokerShared` を mailbox に install する
  - PoisonPill、Kill、Identify、system message 分岐、failure reporting、receive timeout reschedule の観測結果が変わらないことを確認する
  - dispatch facet の sibling test が主要 dispatch 回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.1, 3.1, 3.2, 4.1, 4.2
  - _Boundary:_ Dispatch Facet
  - _Depends:_ 1.1

- [x] 2.2 Children Facet を分離する
  - child registration、child stop、children state predicate、suspend/resume propagation、restart stats を children 責務として切り出す
  - child registry と supervision watch の連携が再編前と同じ結果になることを確認する
  - children facet の sibling test が child stop と subtree propagation の主要回帰を実行できる状態になる
  - _Requirements:_ 1.3, 2.4, 3.1, 4.1, 4.2
  - _Boundary:_ Children Facet
  - _Depends:_ 1.1

- [x] 2.3 DeathWatch Facet を分離する
  - watch/unwatch、supervision watch、watch_with、terminated dedup、DeathWatchNotification delivery を death watch 責務として切り出す
  - user watch と supervision-only watch の違い、custom terminated message、`on_terminated` callback が再編前と同じ結果になることを確認する
  - death watch facet の sibling test が watch/unwatch と terminated dedup の主要回帰を実行できる状態になる
  - _Requirements:_ 1.4, 2.4, 3.1, 4.1, 4.2
  - _Boundary:_ DeathWatch Facet
  - _Depends:_ 2.2

- [x] 2.4 FaultHandling Facet を分離する
  - failure state、failure reporting、fault recreate、finish recreate、child failure directive、failure outcome recording を fault handling 責務として切り出す
  - mailbox suspension、children suspension、restart/resume/escalate/stop directive、fatal failure の観測結果が再編前と同じになることを確認する
  - fault handling facet の sibling test が restart/resume/escalate/fatal failure の主要回帰を実行できる状態になる
  - _Requirements:_ 1.2, 1.3, 2.3, 3.1, 4.1, 4.2
  - _Boundary:_ FaultHandling Facet
  - _Depends:_ 2.2, 2.3

- [x] 2.5 Lifecycle Facet を分離する
  - create、stop、finish terminate、lifecycle event publication、guardian/system termination を lifecycle 責務として切り出す
  - actor stop 時の child termination 待ち、watcher notification、name release、cell removal が再編前と同じ順序で完了することを確認する
  - lifecycle facet の sibling test が create/stop/guardian termination の主要回帰を実行できる状態になる
  - _Requirements:_ 1.2, 2.2, 3.1, 4.1, 4.2
  - _Boundary:_ Lifecycle Facet
  - _Depends:_ 2.3, 2.4

- [x] 2.6 ReceiveTimeout Facet を分離する
  - user message 成功後の receive timeout reschedule 判定と lifecycle/restart/stop 時の cancel 経路を receive timeout 責務として切り出す
  - `NotInfluenceReceiveTimeout` 相当の message flag が再編前と同じ reschedule 抑止結果になることを確認する
  - receive timeout facet の sibling test が cancel/reschedule の主要回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.5, 3.1, 4.1, 4.2
  - _Boundary:_ ReceiveTimeout Facet
  - _Depends:_ 2.1, 2.5

- [x] 2.7 Stash Facet を分離する
  - stash/unstash、容量超過、rollback を stash 責務として切り出す
  - mailbox enqueue failure の観測結果と stashed message の復元結果が再編前と同じになることを確認する
  - stash facet の sibling test が stash/unstash の主要回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.5, 3.1, 3.4, 4.1, 4.2
  - _Boundary:_ Stash Facet
  - _Depends:_ 2.1, 2.5

- [x] 2.8 Timer Facet を分離する
  - single/fixed-delay/fixed-rate timer、active 判定、single/all cancel を timer 責務として切り出す
  - scheduler error と cancellation の観測結果が再編前と同じになることを確認する
  - timer facet の sibling test が timer scheduling/cancel の主要回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.5, 3.1, 3.4, 4.1, 4.2
  - _Boundary:_ Timer Facet
  - _Depends:_ 2.5

- [x] 2.9 PipeTask Facet を分離する
  - pipe_to_self/pipe_to task の登録、poll、delivery、cleanup を pipe task 責務として切り出す
  - terminated actor、target delivery failure、self delivery failure の観測結果が再編前と同じになることを確認する
  - pipe task facet の sibling test が pipe delivery の主要回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.5, 3.1, 3.4, 4.1, 4.2
  - _Boundary:_ PipeTask Facet
  - _Depends:_ 2.5

- [x] 2.10 AdapterHandle Facet を分離する
  - adapter handle の採番、remove、drop 時 stop notification を adapter handle 責務として切り出す
  - adapter sender の停止通知と cleanup 結果が再編前と同じになることを確認する
  - adapter handle facet の sibling test が adapter handle stop/drop の主要回帰を実行できる状態になる
  - _Requirements:_ 1.1, 2.5, 3.1, 3.4, 4.1, 4.2
  - _Boundary:_ AdapterHandle Facet
  - _Depends:_ 2.5

- [x] 3. 統合
- [x] 3.1 root ActorCell と facet tests を整理する
  - root ActorCell test は creation/accessor/module integration の最小回帰へ縮小し、移動したシナリオは各 facet test から実行できるようにする
  - root ActorCell が root orchestration と module wiring に集中し、1,000 行未満であることを確認する
  - `actor.rs` の public re-export が増えていないことを確認する
  - _Requirements:_ 3.2, 4.1, 4.2, 4.4
  - _Boundary:_ ActorCell Facet Tests, ActorCell Core
  - _Depends:_ 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10

- [x] 4. 検証
- [x] 4.1 ActorCell 周辺の targeted test を通す
  - actor_cell、actor_context、dispatcher/mailbox pressure 周辺の targeted unit test を実行する
  - 分割後のすべての facet test が green になり、既存主要シナリオが失われていないことを確認する
  - test failure がある場合は責務境界に沿って原因を戻し、root ActorCell へ ad hoc 分岐を戻さない
  - _Requirements:_ 1.1, 1.2, 1.3, 1.4, 4.1, 4.2, 4.3
  - _Boundary:_ 全体検証
  - _Depends:_ 3.1

- [x] 4.2 構造 lint、clippy、no_std を通す
  - fmt、構造 lint、clippy、no_std check を実行し、facet module と sibling test 配置が project rules に従っていることを確認する
  - `actor_cell.rs` が leaf module のまま残り、`actor.rs` の `pub use actor_cell::ActorCell` が `module-wiring-lint` に通ることを確認する
  - private module、`pub(super)` helper、new `pub(crate)` / `pub use` 禁止の visibility 契約が守られていることを確認する
  - 新しい public trait/helper type、直接 std/Arc/Mutex、不要な内部可変性が追加されていないことを確認する
  - actor-core-kernel の targeted verification が exit 0 で完了する
  - _Requirements:_ 3.2, 3.3, 3.4, 4.3, 4.4
  - _Boundary:_ 全体検証
  - _Depends:_ 4.1

## Implementation Notes

- 1.1: `module-wiring-lint` は leaf module の直属親 re-export のみを許可するため、`actor_cell.rs` は child module を持たない leaf module として残し、facet は `actor_cell_dispatch.rs` 形式の `actor` 直下 private sibling module にする。
