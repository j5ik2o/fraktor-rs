# 調査・設計判断

## 要約
- **機能**: `cluster-ddata-core-types`
- **ディスカバリー範囲**: 新規機能（独立した新規 `ddata` モジュール）
- **主要な発見**:
  - Pekko の `PNCounterMap` は `ORMap`（observed-remove + `VersionVector`）に依存するが、本スペックは `ORMap`/`VersionVector` を対象外とするため、`PNCounterMap` は observed-remove を持たない grow-only キーモデルに限定する必要がある。
  - ノード識別 `UniqueAddress` は `fraktor_remote_core_rs::address` が所有し、`Ord`/`Hash` 導出済みで `BTreeMap` キーに直接使える。新たな識別子型は作らない。
  - `cluster-core-kernel` に proptest は未導入のため、merge 法則検証用に `[dev-dependencies]` へ追加が必要。
  - Pekko の `ReplicatedDelta::zero` は delta 型ではなく空の full state を返す契約である。Rust 側も `Delta != Full` の将来型で公開 SPI を変えないよう、`ReplicatedDelta::Full` を明示する必要がある。

## 調査ログ

### Pekko Distributed Data の公開契約
- **背景**: CRDT 基底 SPI と基本型を参照実装に忠実に逆輸入するため。
- **参照した情報源**: `references/pekko/distributed-data/.../ddata/{ReplicatedData,Flag,GCounter,PNCounter,PNCounterMap,Key,DistributedData,Replicator}.scala`。
- **発見**:
  - `ReplicatedData` は自己参照型 `type T` と `merge(that: T): T` のみ。`DeltaReplicatedData` が `delta`/`mergeDelta`/`resetDelta`、`ReplicatedDelta` が `zero`、`RemovedNodePruning` が `modifiedByNodes`/`needPruningFrom`/`prune`/`pruningCleanup`。
  - `Flag` は enable-only（true 優先 merge、ノード状態なし）。`GCounter` は per-node max、`PNCounter` は P/N 2 つの GCounter、`PNCounterMap` は `ORMap[A, PNCounter]`。
  - `Key[T]` は id 文字列 + phantom 型で id のみ等価。`SelfUniqueAddress` は `UniqueAddress` の薄いラッパで、カウンタ更新が自ノードを明示引数で受けるため。
  - 整合性レベルは Read/Write それぞれ Local / From(n)/To(n) / Majority / MajorityPlus / All（timeout・additional・minCap を保持）。補助 protocol は `GetReplicaCount`/`ReplicaCount(n)`/`FlushChanges`。
  - merge は monotonic join で、結合・可換・冪等が CvRDT の要件。
- **含意**: 基底 SPI の Scala `type T` は Rust の `Self` に対応づけ、`merge(&self, other: &Self) -> Self` とする。`ReplicatedDelta::zero` は delta ではなく full state の空値を返す associated type 付き契約にする。`PNCounterMap` の OR 削除と delta は本スペック境界外。

### cluster-core-kernel の既存パターン
- **背景**: 新規モジュールを既存規約に整合させるため。
- **参照した情報源**: `cluster-core-kernel/src/lib.rs`、`membership.rs`（wiring）、`membership/{membership_version,node_status}.rs`、`remote-core/src/address/unique_address.rs`、`actor-core-kernel` の proptest 例。
- **発見**: トップレベルは `pub mod`、mod.rs 禁止。値型は `const fn` + CQS + 内部可変性なし。`UniqueAddress` は `Clone+Eq+Ord+Hash`。コレクションは `alloc::collections::{BTreeMap,BTreeSet}`（hashbrown 不使用）。proptest は cluster-core-kernel 未導入。
- **含意**: `ddata.rs` wiring + `ddata/*.rs`、`BTreeMap<UniqueAddress, u128>` で per-node 状態、proptest を dev-dependency 追加。bounded arithmetic の overflow は `CounterArithmeticError` で明示的に拒否する。

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|------|------|--------------|------|
| Trait SPI + immutable value | merge を pure 関数として `&self -> Self` | CRDT 法則検証が容易、内部可変性なし、CQS 整合 | 返却ごとに clone/allocation | 採用 |
| `&mut self` mutator | in-place 更新 | allocation 削減 | 複数バージョン保持・merge と相性が悪く、法則テストが書きにくい | 不採用 |
| `self` 消費 merge | `merge(self, other) -> Self` | clone 削減 | 両オペランド保持が必要なテストで不便 | 不採用 |

## 設計判断

### 判断: merge / 更新 API を immutable value（`&self -> Self`）にする
- **背景**: brief が「merge は self 消費 or `&mut self` を design で確定」と要求。`cqs-principle` / `immutability-policy` との整合が必要。
- **検討した代替案**: 1) `&mut self` mutator、2) `self` 消費 merge。
- **採用したアプローチ**: `fn merge(&self, other: &Self) -> Self` と `fn increment(&self, ...) -> Self`。self を変更しない pure 関数。
- **根拠**: self を変更しないため CQS の Query に該当し違反なし。内部可変性を使わないため `immutability-policy` に整合。CRDT 法則の property test（`a.merge(b)` と `b.merge(a)` を同一値で評価）が書きやすい。
- **トレードオフ**: 返却ごとに allocation が発生するが、core の正当性・検証容易性を優先。
- **フォローアップ**: property test で merge 法則を検証。ホットパスが判明した場合のみ将来 `&mut self` 版を追加検討。

### 判断: PNCounterMap を grow-only キーモデルに限定（observed-remove 非対象）
- **背景**: Pekko の `PNCounterMap` は `ORMap`（`VersionVector`）依存だが、本スペックは `ORMap`/`VersionVector` 対象外。
- **検討した代替案**: 1) PNCounterMap 全体を Phase 2 へ延期、2) 最小の causal tracking を内蔵。
- **採用したアプローチ**: キー集合を grow-only union とし、conflict-free 削除（observed-remove）と delta を持たない `ReplicatedData` + `RemovedNodePruning` 実装に限定。
- **根拠**: grow-only キー + per-key PNCounter merge は妥当な CvRDT で、`VersionVector` なしに収束する。brief が PNCounterMap を In、`VersionVector`/`ORMap` を Out としている矛盾を、削除意味論の縮小で解消。
- **トレードオフ**: `remove` を提供しない。OR 削除は Phase 2 の OR/LWW スペックへ委譲。
- **フォローアップ**: Phase 2 で `ORMap` 導入時に observed-remove と delta を追加。

### 判断: per-node 値を u128 とし、overflow は明示的に拒否する
- **背景**: Pekko は BigInt（任意精度）。no_std core で bignum 依存は重い。
- **検討した代替案**: 1) 任意精度整数 crate を導入、2) saturating arithmetic、3) wraparound を許容。
- **採用したアプローチ**: per-node `u128`、更新・値集計・プルーニング畳み込みは `checked_*` で検査し、範囲外は `CounterArithmeticError::Overflow` を返す。`GCounter::value -> Result<u128, _>`、`PNCounter::value -> Result<i128, _>` とする。
- **根拠**: wraparound は grow-only 不変条件を壊し、saturating は利用者が期待するカウンタ値を無音で丸める。任意精度整数は Phase 1 の pure core vocabulary には依存追加が重いため、bounded arithmetic を明示する。
- **トレードオフ**: API が `Result` を返す。将来 bignum 化する場合は `CounterArithmeticError` と value API の再検証が必要。

### 判断: `ReplicatedDelta::zero` は full state associated type を返す
- **背景**: Phase 1 の `GCounter` / `PNCounter` は `Delta = Self` だが、Phase 2 の OR 系 delta は full state と delta 型が分かれる。
- **検討した代替案**: 1) Phase 1 に合わせ `zero(&self) -> Self` とする、2) Phase 2 で trait を破壊的に変更する。
- **採用したアプローチ**: `ReplicatedDelta` に `type Full: DeltaReplicatedData<Delta = Self>` を持たせ、`zero(&self) -> Self::Full` とする。
- **根拠**: Pekko の `zero` は delta ではなく空の full state を返す契約であり、将来 `Delta != Full` の型を追加しても SPI を変えずに済む。
- **トレードオフ**: Phase 1 の実装では associated type が同型を指すだけなので少し冗長。

### 判断: pruning は data-local 失敗だけを trait で返し、分散状態管理は後続 Replicator が所有する
- **背景**: Pekko は `DataEnvelope` / `PruningState` が pruning transition と遅延 update の cleanup を所有する。本スペックは Replicator runtime を対象外にしている。
- **検討した代替案**: 1) Phase 1 に envelope/pruning state を含める、2) `RemovedNodePruning::prune` を常に infallible にする。
- **採用したアプローチ**: `RemovedNodePruning` は data-local な `PruneError` associated type を持ち、overflow などの畳み込み不能を返す。pruned/unpruned 混在 merge の安全性、retry、report は後続 Replicator envelope が所有する。
- **根拠**: data 型だけで分散 pruning transition を完結させると責務境界が崩れる。一方、bounded arithmetic の失敗は data-local に検出できるため trait に出す。
- **トレードオフ**: 後続 Replicator spec は `prune` の `Result` を明示的に扱う必要がある。

## リスクと緩和策
- delta の assoc 型循環（`DeltaReplicatedData::Delta: ReplicatedDelta<Full = Self>`、`ReplicatedDelta::Full: DeltaReplicatedData<Delta = Self>`）→ Phase 1 は `Delta = Self`（GCounter/PNCounter）で検証し、Phase 2 の `Delta != Full` 追加時に compile-time 契約を再確認する。
- property test の `UniqueAddress` 生成コスト → 少数の固定ノード集合上で操作列を生成し、決定的 `BTreeMap` で値等価を比較。
- `PNCounterMap` の境界誤解（OR 削除が無い）→ design・requirements・rustdoc で明示し、Phase 2 委譲を記録。
- counter overflow の境界見落とし → 最大値近傍の更新・値集計・プルーニング畳み込みを unit/property test に含め、wraparound と saturating を禁止する。

## 参考資料
- `references/pekko/distributed-data/src/main/scala/org/apache/pekko/cluster/ddata/*.scala` — CRDT 基底 SPI・基本型・整合性レベルの参照実装。
- `.kiro/specs/cluster-ddata-core-types/brief.md` — discovery によるスコープ・境界判断。
- `.agents/rules/rust/{cqs-principle,immutability-policy,type-organization,naming-conventions,reference-implementation}.md` — 設計制約。
