# Batch 3: `ConsistentHashingRoutingLogic` 判定クロージング

## 概要

pekko-porting ワークフローの Batch 3 は、Phase 2 medium の
「`ConsistentHashingRoutingLogic` 完全化」系 3 項目
（`ConsistentHashingRoutingLogic`（partial）/ `ConsistentHash<T>` / `MurmurHash` util /
`virtualNodesFactor` パラメータ）を **判定クロージング** する。

機能追加はゼロ。rendezvous hashing を採用している現行実装が Pekko 契約を満たしていること
を確認し、その判定根拠を本ドキュメントに永続化する。

本ドキュメントは `docs/gap-analysis/actor-gap-analysis.md`（第6版）からリンクされる
唯一の判定根拠ドキュメントとして機能する。

## 参照資料

| 参照対象 | パス |
|----------|------|
| Pekko `ConsistentHashingRoutingLogic` | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/ConsistentHashing.scala` |
| Pekko `ConsistentHash[T]` | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/ConsistentHash.scala` |
| Pekko `MurmurHash` | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/MurmurHash.scala` |
| fraktor-rs 実装 | `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs` |
| fraktor-rs Pool | `modules/actor-core/src/core/kernel/routing/consistent_hashing_pool.rs` |
| fraktor-rs Envelope | `modules/actor-core/src/core/kernel/routing/consistent_hashable_envelope.rs`（Batch 1 成果） |
| fraktor-rs Hashable | `modules/actor-core/src/core/kernel/routing/consistent_hashable.rs` |
| 設計ルール | `.agents/rules/rust/immutability-policy.md`, `.agents/rules/rust/reference-implementation.md`, `.agents/rules/rust/naming-conventions.md` |

## Pekko の契約意図（ユーザー可視）

Pekko `ConsistentHashingRoutingLogic` がユーザーに保証する契約は以下の 4 点。

1. **Stable mapping** — 同一 hash key は同一 routee に決定論的にマップされる
   （routee 集合が変化しない限り）。
2. **Minimal disruption** — routee を 1 つ追加／除去したとき、移動するキーの
   期待比率は `1/(n+1)`（追加）／`1/n`（除去）。
3. **Hash key precedence** — `ConsistentHashableEnvelope` → `ConsistentHashable` trait
   → `hashMapping` fallback の 3 段階。
4. **Empty routees** — 空 routee 集合のときは `NoRoutee` を返して panic しない。

この 4 点が「契約意図」であり、これらを Rust で再表現できていれば Pekko 互換の
目的は達成される。実装構造を Pekko と揃えること自体は目的ではない
（`.agents/rules/rust/reference-implementation.md` の「最小 API」方針）。

## Pekko 側の内部実装要素

Pekko の `ConsistentHashingRoutingLogic` が上記契約を実現するために内部で採用している
要素（=「契約意図」ではなく「実装詳細」）:

| 要素 | 役割 |
|------|------|
| `ConsistentHash[T]`（`SortedMap[Int, T]`） | virtual node × routee を int hash でソートし、clockwise に最初のノードを選ぶリング |
| `virtualNodesFactor` | 1 routee あたりの virtual node 数。リング分散の均一化に使う |
| `MurmurHash`（MurmurHash3 32-bit） | ring 構築と key 算出に使うハッシュ関数 |
| `AtomicReference[(IndexedSeq[Routee], ConsistentHash)]` | `routees` が変わらない限り再構築を避けるキャッシュ |
| `ConsistentRoutee`（`selfAddress` 付き wrapper） | クラスタ remote routee が address を共有するためのラッパー |
| `ConsistentHashMapping`（`PartialFunction[Any, Any]`） | 任意メッセージから hash key を抽出する user-supplied マッパー |

これらは **「契約意図」ではなく「リング方式を採用したときに必要な実装詳細」** である。
別の選択肢（rendezvous hashing, consistent hashing with jump hash 等）を採れば、
この要素集合は丸ごと不要になる。

## fraktor-rs 現行実装との構造差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| 選択アルゴリズム | sorted hash ring + virtual nodes | rendezvous hashing (HRW) + FNV mix |
| 状態 | `AtomicReference` にリングキャッシュ | stateless (`&self`) |
| `hashMapping` 抽象 | `PartialFunction[Any, Any]` | `hash_key_mapper: ArcShared<dyn Fn(&AnyMessage) -> u64 + Send + Sync>` |
| Envelope | `ConsistentHashableEnvelope` | `ConsistentHashableEnvelope`（Batch 1 で採用済み） |
| Hashable trait | `ConsistentHashable` | `ConsistentHashable` |
| Address-aware wrapper | `ConsistentRoutee`（selfAddress 付き） | 不要（`Pid::new(value, generation)` で一意） |

## rendezvous hashing が契約 1〜4 を満たすことの確認

rendezvous hashing（Highest Random Weight, HRW; Thaler & Ravishankar 1998）は、
各キーに対し `(key, routee_identity)` の組み合わせハッシュを全 routee 分計算し、
スコアが最大の routee を選ぶアルゴリズム。

### 契約 1: Stable mapping ✅

キー `k` と routee 集合 `R` が固定なら、`argmax_{r ∈ R} score(k, r)` は決定論的に
同じ routee を返す。`score` が純粋関数であることから自明。

実装位置: `consistent_hashing_routing_logic.rs:select` の `max_by_key`。
検証テスト: `select_same_hash_key_returns_same_routee`,
`select_is_stable_across_routee_order_changes`。

### 契約 2: Minimal disruption ✅

- **追加** (`n → n+1`): 新 routee `r'` が `argmax` になるキーは、`score(k, r') > max_{r ∈ R} score(k, r)` を満たすキーのみ。`score` が一様ランダムであることから、その比率は `1/(n+1)`。
- **除去** (`n → n-1`): 除去された routee `r_d` を `argmax` にしていたキーだけが移動する。その比率は `1/n`。

この性質は `ConsistentHash` の sorted ring でも「virtual node が無限個の極限で」
`1/n` に収束する。rendezvous hashing は virtual node に相当する冗長ノードを
必要とせず **構造上均一** に近似できる点で、`virtualNodesFactor` を省略できる。

実装位置: `consistent_hashing_routing_logic.rs:select` の `rendezvous_score` × `max_by_key`。
検証テスト: `select_minimal_disruption_on_routee_addition`（10,000 キー、移動比率 ≈ 0.25 ± 0.05）,
`select_minimal_disruption_on_routee_removal`（10,000 キー、移動比率 ≈ 0.25 ± 0.05）。
両テストは Batch 3 の `write_tests` ステップで追加済み。

### 契約 3: Hash key precedence ✅

`select` 内で `message.downcast_ref::<ConsistentHashableEnvelope>()` を先に試行し、
マッチしなければ `hash_key_mapper` へフォールバックする 2 段階経路。
`ConsistentHashable` trait の扱いは Envelope 内で吸収されており、
ユーザー可視の 3 段 precedence（Envelope → Hashable → mapper）は
Envelope が `ConsistentHashable` を impl する構造で達成されている。

実装位置: `consistent_hashing_routing_logic.rs:select` の `if let Some(envelope) = ... else ...`。
検証テスト: `envelope_hash_key_takes_precedence_over_mapper`,
`envelope_with_same_hash_key_selects_same_routee`,
`no_envelope_falls_back_to_mapper`。

### 契約 4: Empty routees ✅

`routees.is_empty()` を早期 return で検出し、`Routee::NoRoutee` を返す。

実装位置: `consistent_hashing_routing_logic.rs:select` の `if routees.is_empty() { return &NO_ROUTEE; }`。
検証テスト: `select_empty_routees_returns_no_routee`。

## 判定結果

| Pekko 要素 | 判定 | 根拠 |
|------------|------|------|
| `ConsistentHashable` trait | **採用済み**（Batch 前） | `consistent_hashable.rs:10-13` に既存 |
| `ConsistentHashableEnvelope` | **採用済み**（Batch 1） | `consistent_hashable_envelope.rs` |
| `ConsistentHashMapping` (hash key 抽出抽象) | **翻訳済み** | `hash_key_mapper: ArcShared<dyn Fn(&AnyMessage) -> u64 + Send + Sync>` で Rust 自然形に変換済み |
| `ConsistentHashingRoutingLogic` の選択アルゴリズム | **翻訳** | rendezvous hashing で契約 1〜4 を全て満たす。stateless `&self` / no_std / 型数最小化の全てに整合 |
| `ConsistentHash<T>` 公開 util | **非採用（n/a）** | rendezvous では ring 自体が不要。Pekko 内部実装の複製になる |
| `MurmurHash` 公開 util | **非採用（n/a）** | rendezvous では FNV ベース `mix_hash` で契約充足。Pekko 内部実装の複製になる |
| `virtualNodesFactor` パラメータ | **非採用（n/a）** | ring 固有概念。rendezvous では意味を持たない。no-op な knob になって利用者を誤誘導する |
| `AtomicReference` routees キャッシュ | **非採用（n/a）** | stateless 実装で不要。`.agents/rules/rust/immutability-policy.md` の内部可変性禁止にも合致 |
| `ConsistentRoutee`（selfAddress 付き wrapper） | **非採用（n/a）** | `Pid::new(value, generation)` で一意性を保証済み。wrapper 不要 |

### 設計ルールとの整合

- `.agents/rules/rust/immutability-policy.md`: stateless `&self` で AShared パターンを避けられる ✅
- `.agents/rules/rust/reference-implementation.md`:
  - Go / Scala → Rust 変換で言語特性を尊重 ✅
  - YAGNI: 使われない virtual node / ring キャッシュを作らない ✅
  - 型数目安（fraktor-rs ≤ 参照実装の 1.5 倍）: Pekko が 6 型追加するのに対し fraktor-rs は 0 型追加 ✅
- `.agents/rules/rust/naming-conventions.md`: rustdoc 英語 ✅
- `CLAUDE.md`（本プロジェクト）:
  - 後方互換不要のため rendezvous 採用に支障なし ✅
  - 計画ドキュメントは `docs/plan/` 配下 ✅
  - lint `#[allow]` 回避禁止: 該当なし ✅
  - TOCTOU 回避: stateless 実装で元から TOCTOU フリー ✅

## 成果物

### プロダクションコード

- `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs`
  - 型 / `select` の rustdoc に Pekko 契約 1〜4、rendezvous 採用理由、非採用要素の
    理由（`ConsistentHash<T>` / `MurmurHash` / `virtualNodesFactor` /
    `AtomicReference` / `ConsistentRoutee`）を英語で追記
- `modules/actor-core/src/core/kernel/routing/consistent_hashing_pool.rs`
  - 型 rustdoc に `with_virtual_nodes_factor` を提供しない理由を英語で追記

### テスト

- `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic/tests.rs`
  - `select_minimal_disruption_on_routee_addition`（10,000 キー、±0.05）
  - `select_minimal_disruption_on_routee_removal`（10,000 キー、±0.05）
  - Batch 3 の `write_tests` ステップで追加済み

### ドキュメント

- `docs/gap-analysis/actor-gap-analysis.md` を第6版に更新
  - `ConsistentHashingRoutingLogic` の partial 扱いを「完全実装（翻訳）」に昇格
  - `ConsistentHash<T>` / `MurmurHash` / `virtualNodesFactor` を n/a（非採用）に分類
  - サマリー / 層別カバレッジ / Phase 2 セクションを再計算
- `docs/plan/pekko-porting-batch-3-consistent-hashing.md`（本ドキュメント）

## スコープ外

| 項目 | 理由 |
|------|------|
| `OptimalSizeExploringResizer`（Phase 3 hard） | Batch 4 予定。kernel に `Resizer` trait が無く、trait 移設を含む別バッチで対応 |
| receptionist / delivery internal 分離（Phase 2 medium） | Batch 5 予定。typed 側の内部整理で kernel→typed 順では後工程 |
| `ConsistentHashingGroup` kernel 新設 | gap analysis に記載なし。typed `GroupRouter::with_consistent_hash_routing` は kernel `ConsistentHashingRoutingLogic` を直接利用する経路で充足済み |

## 未来の判定変更トリガ

現行判定は rendezvous hashing の性質と fraktor-rs の設計原則を前提としている。
以下のいずれかが発生した場合は再判定を行うこと。

1. クラスタ remote routee を `selfAddress` 等のローカル情報と結びつける必要が生じた
   → `ConsistentRoutee` 相当の wrapper が必要になる可能性
2. 極端に不均一なハッシュ分布を持つカスタム `hash_key_mapper` が主要ユースケースになった
   → virtual node 類似の均一化が必要になる可能性
3. Pekko `ConsistentHashingRoutingLogic` の契約 1〜4 が変化した
   → 契約追随で実装方針を再検討

いずれも現時点では発生していない。
