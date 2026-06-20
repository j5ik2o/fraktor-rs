# 調査・設計判断

## 要約
- **機能**: `actor-system-state-registry-split`
- **ディスカバリー範囲**: 既存 system state への拡張 / 内部構造整理
- **主要な発見**:
  - `SystemState` は actor identity、cell/name registry、guardian、event/logging、dispatcher/mailbox、remote/provider、scheduler/shutdown を同一型に保持している。
  - `SystemStateShared` は `event_stream`、`dead_letter`、`cells`、remote hook、scheduler などを cached handle として保持しており、分割後も handle 同一性を保つ必要がある。
  - 既存の `CellsShared`、`ActorPathRegistry`、`RemoteAuthorityRegistry`、`Registries` には registry 分離の先行パターンがあるため、新しい直接 `Arc` / `Mutex` は不要である。

## 調査ログ

### SystemState の責務集中
- **背景**: mailbox / EventBus / shutdown の後続 spec が同じ `SystemState` を変更すると、無関係な責務へ差分が波及する。
- **参照した情報源**: `modules/actor-core-kernel/src/system/state/system_state.rs`、`modules/actor-core-kernel/src/system/state/system_state_shared.rs`
- **発見**:
  - `SystemState` は build/config 適用、actor path、name registry、guardian、extra top-level、temp actor、event stream、extensions、remote authority、dispatcher/mailbox、scheduler、failure counters を一箇所で扱う。
  - `SystemStateShared` は thread-safe façade として多数の委譲メソッドを持ち、一部の handle を lock 外で clone 可能にしている。
- **含意**: `SystemState` の public / crate-visible accessor は維持しつつ、内部 field と関連 helper を private registry 型へ移す設計が最小変更になる。

### 既存 registry / shared wrapper パターン
- **背景**: shared state への直接同期 primitive 追加を避ける必要がある。
- **参照した情報源**: `modules/actor-core-kernel/src/system/cells.rs`、`modules/actor-core-kernel/src/system/cells_shared.rs`、`modules/actor-core-kernel/src/system/registries.rs`、`.kiro/steering/tech.md`
- **発見**:
  - `CellsShared` は `SharedLock<Cells>` と `SharedAccess` を使い、registry 自体は単一責務の private 型になっている。
  - project steering は `ArcShared`、`Shared*`、closure-based API を優先し、直接 `Arc` / `Mutex` を避ける方針を明示している。
- **含意**: 新 registry は `SystemState` の内部所有型として始め、共有が必要なものだけ既存 `Shared*` pattern を使う。

### 後続 spec との接続点
- **背景**: この spec は EventBus / mailbox / CoordinatedShutdown の前提 workstream である。
- **参照した情報源**: `.kiro/steering/roadmap.md`、`.kiro/specs/actor-system-state-registry-split/brief.md`
- **発見**:
  - `actor-eventbus-classification-contract` と `actor-mailbox-resolution-contract` は state 境界の安定化後に変更面を狭くできる。
  - `actor-coordinated-shutdown-task-variants` は scheduler / shutdown coordination state の境界を前提にできる。
- **含意**: registry 名と設計境界は後続 spec が参照できる粒度で記録するが、後続 spec の新しい挙動は取り込まない。

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|------|------|--------------|------|
| Private leaf registry 抽出 | `SystemState` 配下に private registry 型を追加し、既存 accessor を委譲する | 外部 API を維持しやすい。後続 spec の変更面を狭くできる | 初回は委譲メソッドが増える | 採用 |
| Public registry handle 公開 | subsystem ごとの registry handle を外部 crate へ公開する | 利用側が直接 registry を扱える | public surface audit 前に API を増やす | 不採用 |
| 現状維持 | `SystemState` の field 整理だけに留める | 差分が少ない | mailbox / EventBus / shutdown の競合を解消しない | 不採用 |

## 設計判断

### 判断: SystemState は public façade、registry は private leaf にする
- **背景**: 既存 actor / typed / remote の呼び出し元は `SystemStateShared` と既存 accessor に依存している。
- **検討した代替案**:
  1. registry handle を public にする。
  2. `SystemState` の field を private registry へ移し、既存 accessor を維持する。
- **採用したアプローチ**: private registry 型を `system/state/` に置き、`SystemState` は構築・委譲・互換 façade を担う。
- **根拠**: `actor-kernel-public-surface-audit` が別 spec であるため、この spec では public surface を増やさない。
- **トレードオフ**: 既存 accessor の委譲コードは残るが、後続の registry 内部変更は局所化できる。
- **フォローアップ**: 実装時に public re-export が増えていないことを確認する。

### 判断: registry 境界は後続 spec の変更軸に合わせる
- **背景**: 変更競合の主因は mailbox / EventBus / shutdown workstream が同じ巨大 state 型を触ることである。
- **検討した代替案**:
  1. field 数だけで機械的に registry を分ける。
  2. 後続 spec の責務境界に合わせて registry を分ける。
- **採用したアプローチ**: dispatcher / mailbox、event / logging、guardian / cells、remote / provider / deployment、scheduler / shutdown、identity / path を主要境界にする。現行 `SystemState` に serializer registry 本体はないため、serialization は deployment/provider setup に隣接する境界として扱い、新しい serializer 公開契約は作らない。
- **根拠**: roadmap の downstream spec と一対一に近い変更面を作れる。
- **トレードオフ**: serialization そのものの新契約は扱わず、現行 state に存在する deployable factory / remote hook の範囲に留める。
- **フォローアップ**: EventBus trait 族や mailbox resolution は後続 spec 側で追加する。

### 判断: 共有化は必要な handle だけに限定する
- **背景**: `SystemStateShared` は cached handle の同一性を期待している。
- **検討した代替案**:
  1. すべての registry を `SharedLock` 化する。
  2. 既存 shared handle を維持し、`SystemState` lock 内でよい registry は private owned にする。
- **採用したアプローチ**: `CellsShared`、`EventStreamShared`、remote hook、scheduler など既存 shared handle は維持し、それ以外は `SystemState` 内部所有にする。
- **根拠**: lock 階層と shared surface を増やさず、`no_std` / closure-based access 方針に沿う。
- **トレードオフ**: registry ごとの独立 lock は作らないため、並行性能改善は目的外になる。
- **フォローアップ**: read-then-act を増やす変更がないか実装レビューで確認する。

## リスクと緩和策
- cached handle の clone 経路が変わる — `SystemStateShared::new` / `from_shared_rw_lock` と typed facade tests で同等性を確認する。
- registry 抽出が過分割になる — 後続 spec の変更軸に対応する境界だけを作り、単一 field のためだけの wrapper は避ける。
- module wiring lint に抵触する — `system/state.rs` を module 宣言と re-export に留め、型ごとに sibling file と sibling test を置く。

## 参考資料
- `.kiro/steering/product.md` — actor runtime の目的と core / adaptor 分離。
- `.kiro/steering/tech.md` — `no_std`、shared abstraction、closure-based access 方針。
- `.kiro/steering/structure.md` — module crate と file organization 方針。
- `.kiro/steering/roadmap.md` — actor Phase 3 の依存順と downstream spec。

---

## Gap Validation: 2026-06-20

### Analysis Summary
- `SystemState` は 1147 行、`SystemStateShared` は 1094 行で、どちらも 1k 行を超えており、registry split の必要性は実装面でも確認できる。
- 現行 state には design 初稿の 6 registry だけでは回収されない runtime support state（pid allocation、monotonic clock、ask futures、extensions、invoke guard、circuit breaker config）がある。
- children relation は `SystemState` ではなく `ActorCell` の children facet が所有しているため、`GuardianCellRegistry` が children state を所有する設計は境界違反になる。
- serializer registry 本体は `SystemState` に存在せず、現行 state が持つのは deployment/provider setup と remote hook に隣接する状態である。
- 外部 dependency 追加は不要で、既存 `ArcShared` / `Shared*` / `SharedAccess` pattern と private sibling module で実装できる。

### Requirement-to-Asset Map

| Requirement | Existing assets | Gap |
|-------------|-----------------|-----|
| 1.1, 1.2, 1.3 | `system_state.rs`, `system_state_shared.rs`, typed `TypedActorSystem` / `Dispatchers` façade | Missing: façade の戻り値を維持したまま private registry へ委譲する抽出 |
| 2.1 | `SystemState` field 群、`CellsShared`, `Registries`, `ActorPathRegistry`, `RemoteAuthorityRegistry` | Missing: runtime support / dispatch / event / guardian / remote / scheduler の境界明文化 |
| 2.2 | `Dispatchers`, `Mailboxes`, `MailboxSharedSet` | Missing: dispatch/mailbox 専用 registry と代表テスト |
| 2.3 | `EventStreamShared`, `DeadLetterShared`, `LoggingFilter`, failure counters | Missing: event/logging 専用 registry と filter / publish / failure outcome テスト |
| 2.4 | `CellsShared`, `Registries`, `GuardiansState`, `ActorPathRegistry`, `ActorCell` children facet | Constraint: children state は `ActorCell` 所有。SystemState registry は cell table / guardian / path façade に留める |
| 3.1-3.4 | `fraktor_utils_core_rs::sync`, existing shared wrappers | Constraint: 直接 `Arc` / `Mutex` を増やさず、既存 shared abstraction を使う |
| 4.1-4.3 | `actor-core-kernel` no_std crate, custom dylint rules | Constraint: private sibling module と sibling test placement が必要 |
| 5.1-5.3 | `system_state_test.rs`, `system_state_shared_test.rs`, typed system tests | Missing: registry 単位の代表委譲テストと downstream 境界の明示 |

### Implementation Options

#### Option A: Extend Existing Components
- **内容**: `SystemState` の field を少し並べ替え、既存 helper を増やす。
- **強み**: 差分が小さい。
- **弱み**: 1k 行超えファイルの構造問題と後続 spec の競合を解消しない。
- **評価**: requirements 2.1 / 5.3 を満たしにくいため不採用。

#### Option B: Create New Private Registries
- **内容**: private leaf registry を追加し、`SystemState` は façade / construction / cross-registry coordination に縮小する。
- **強み**: 既存 API を維持しながら後続変更面を分割できる。module lint と 1型1ファイル方針に合う。
- **弱み**: 初回の field 移動と委譲テストが多い。
- **評価**: preferred。

#### Option C: Hybrid Extraction
- **内容**: downstream に直結する dispatch/event/remote/scheduler だけを先に切り、support state は `SystemState` に残す。
- **強み**: 初期差分を抑えられる。
- **弱み**: `SystemState` が bundle 役に縮小しきらず、後から support state の再移動が必要になる。
- **評価**: 段階導入としては viable。ただし design では `RuntimeSupportRegistry` を追加し、実装時に PR サイズが過大なら support registry を最後に切る。

### Effort and Risk
- **Effort**: L。2 つの 1k 行超え façade を触り、複数 registry と regression test を追加するため 1-2 weeks 規模。
- **Risk**: Medium。外部依存や新 behavior はないが、cached handle、lock ordering、remote authority event emission、typed façade regression の取りこぼしがあり得る。

### Design Corrections Applied
- `RuntimeSupportRegistry` を design / tasks に追加した。
- `GuardianCellRegistry` から children state 所有を外し、`ActorCell` children facet を正本として明記した。
- `RemoteProviderRegistry` の serialization-adjacent 境界を、serializer registry 新契約ではなく deployment/provider setup 境界として明確化した。
- scheduler / shutdown 境界は root startup gate も扱うため、実装境界名を `SchedulerLifecycleRegistry` に調整した。

### Recommendations for Implementation
- preferred approach は Option B。`SystemState` façade を残しつつ private leaf registry に切る。
- 最初の実装では `RuntimeSupportRegistry` と `IdentityPathRegistry` を先に導入し、constructor duplication を減らしてから downstream-facing registry を切る。
- `SystemStateShared` の cached handle は lock 外 clone の hot path を維持し、`event_stream`, `dead_letter`, `cells`, scheduler, remote hooks を余計な `with_read` 経由に戻さない。
