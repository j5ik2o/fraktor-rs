## 1. Phase 1: 設計探索と合意取得

本 change は **explore / proposal 型** であり、Phase 1 はドキュメント整備のみ。実装変更は Phase 2 (合意後の別 change) で行う。

### 1.A 設計ドキュメント整備

- [x] 1.1 `proposal.md` を作成し、stash と deque mailbox 要求の不整合を整理する (本 change の Why)
- [x] 1.2 `design.md` に 5 つの設計オプション (Option A〜E) を詳述する
  - [x] Option A: Behavior に `mailbox_requirement` field を追加し、spawn 時に Props に merge
  - [x] Option B: `Props::with_mailbox_requirement(MailboxRequirement::for_stash())` を基本とし、typed には薄い convenience を追加 (明示的 opt-in)
  - [x] Option C: Mailbox 側で runtime panic / diagnostics
  - [x] Option D: stash unstash を Behavior layer 内で完結 (mailbox prepend を使わない)
  - [x] Option E: Hybrid (typed = Option D, classic = Option B)
- [x] 1.3 `design.md` に **比較表** を作成し、実装コスト・互換性・Pekko 整合・middleware 経路への影響を可視化する
- [x] 1.4 `design.md` に **recommend 候補** と理由を明示する
- [x] 1.5 `spec.md` に **option-agnostic な不変条件** を記述する (どの option が選ばれても満たすべき contract)

### 1.B 既存挙動の verify

- [x] 1.6 `MailboxRequirement::for_stash()` の caller を grep で確認する (`grep -rn "for_stash" modules/`)
- [x] 1.7 `Behaviors::with_stash` の使用箇所を typed テストで確認する (現状 production で deque 強制されていないことの再確認)
- [x] 1.8 `cell.stash_message_with_limit` の使用箇所を classic テストで確認する
- [x] 1.9 `prepend_via_drain_and_requeue` が production で実行される証拠を test 名で 2 件以上特定する

### 1.C openspec 検証

- [x] 1.10 `openspec validate stash-requires-deque-mailbox --strict` valid を確認

### 1.D commit + push

- [ ] 1.11 commit: `docs(openspec): propose stash-requires-deque-mailbox (explore phase)`
- [ ] 1.12 PR description で「explore / proposal 型 change で実装は含まない」ことを明示
- [ ] 1.13 PR で 5 オプションの review と decision を求める

## 2. Phase 2: 合意後の実装 (本 change の対象外、別 change で実施)

Phase 2 の tasks は **本 change には記述しない**。user / team が Phase 1 の review で option を選んだ後、選ばれた option に対応する別 openspec change として作成される。例:

- 選ばれた option が A → 新規 change `stash-requires-deque-mailbox-via-behavior-field`
- 選ばれた option が B → 新規 change `stash-requires-deque-mailbox-via-props-requirement`
- 選ばれた option が D → 新規 change `stash-via-behavior-runner-direct-replay`
- など

各 Phase 2 change は固有の proposal / design / tasks / spec を持つ。本 change はそれらの **入口 / 設計コンテキスト** として archive される。

### 2.A Phase 1 完了の verify (Phase 2 開始時の前提条件)

- [ ] 2.1 user / team から option 選択の合意を得たことを確認 (PR コメント or 別チャンネル)
- [ ] 2.2 選ばれた option を design.md の Decision 4 に追記 (commit ではなく Phase 2 change の中で)
- [ ] 2.3 Phase 2 用の新規 openspec change を作成
- [ ] 2.4 本 change を archive 候補として記録 (`openspec/changes/archive/` への移動は Phase 2 完了後)

### 2.B 実装 (Phase 2 change の中)

Phase 2 の具体的な task は本 change に書かない。各 Phase 2 change が固有の Migration Plan を持つ。
