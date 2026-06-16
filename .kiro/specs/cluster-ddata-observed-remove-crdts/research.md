# ギャップ分析: cluster-ddata-observed-remove-crdts

更新日: 2026-06-17
対象: `ORSet` / `ORMap` / `ORMultiMap` / `LWWMap` を `modules/cluster-core-kernel/src/ddata/` に追加（gap analysis カテゴリ9 の OR 系 CRDT を閉じる）

## 1. 現状調査（既存資産）

### 再利用できる基盤（実装済み）

| 資産 | 場所 | 本仕様での役割 |
|------|------|----------------|
| CRDT 基底 SPI `ReplicatedData` | `ddata/replicated_data.rs:4` | `fn merge(&self, other: &Self) -> Self`。4 型すべてが実装 |
| `DeltaReplicatedData` | `ddata/delta_replicated_data.rs:6` | `type Delta` + `delta()` / `merge_delta()` / `reset_delta()`（全 `&self`） |
| `ReplicatedDelta` | `ddata/replicated_delta.rs:6` | `type Full` + `zero()` |
| `RequiresCausalDeliveryOfDeltas` | `ddata/requires_causal_delivery_of_deltas.rs:6` | マーカー trait |
| `RemovedNodePruning` | `ddata/removed_node_pruning.rs:10` | `type PruneError` + `modified_by_nodes` / `need_pruning_from` / `prune` / `pruning_cleanup` |
| `PNCounterMap`（observed-remove map の前例） | `ddata/pn_counter_map.rs:22` | per-key dot map + tombstone + removed-values + delta バッファ構造のテンプレート |
| `VersionVector` | `ddata/version_vector.rs:7` | `increment` / `version_at` / `compare`(→`VersionVectorOrdering`) / `entries` / `merge`。ORSet の dot 基盤候補 |
| `LWWRegister<T>` | `ddata/lww_register.rs:18` | `LWWMap` の値型。`new_with_clock` / `with_value_with_clock`（`clock: FnOnce(i64,&T)->i64`）、merge は timestamp 大→同値は `UniqueAddress` 小が勝ち |
| `SelfUniqueAddress` / `Key<T>` 型エイリアス | `ddata/self_unique_address.rs:7`, `ddata/key.rs:9,50` | 自ノード識別引数 / 型タグ key |
| Replicator protocol core | `ddata/replicator_entry.rs:5`, `update.rs:13`, `get.rs:11` | `ReplicatorEntry<D>` / `Get<D,C>` / `Update<D,C>` は generic（`D: ReplicatedData` のみ） |

### 抽出した規約

- **イミュータブル / CQS**: 既存 CRDT は全操作 `&self` → 新インスタンス返却。`&mut self`・内部可変性は不使用（`.agents/rules/rust/immutability-policy.md`、`cqs-principle.md`）。
- **observed-remove の表現**: `PNCounterMap` は `VersionVector` を使わず per-key の `BTreeMap<UniqueAddress, u64>`（dots / removed_dots / removed_values）で因果的 tombstone を表現（`pn_counter_map.rs:22-32`）。
- **delta 形**: `type Delta = Self`（自己参照デルタ）。`delta()` は変更なしで `None`、`reset_delta()` は delta バッファを空にした新インスタンス。
- **構造 lint**: 1 ファイル 1 公開型（`type-per-file-lint`）、`mod.rs` 禁止、sibling `*_test.rs`、親モジュールはバレル集約せず最小 `pub use`（`module-wiring-lint`、`structure.md:58`）。
- **no_std**: `*-core-kernel` は `#![cfg_attr(not(test), no_std)]` + `extern crate alloc`、`std` 直接依存禁止（`tech.md:28`）。
- **参照実装優先**: Pekko の `ORSet`/`ORMap`/`ORMultiMap`/`LWWMap` の収束規則・命名に合わせる（`reference-implementation.md`）。

## 2. 要件 → 資産マップ（ギャップタグ）

| 要件 | 再利用資産 | ギャップ | タグ |
|------|------------|----------|------|
| 1 `ORSet` add-wins / observed-remove | `VersionVector`(dot 候補) + 基底 SPI + PNCounterMap パターン | ORSet 本体（per-element dot、add-wins merge、`subtractDots` 相当）が未実装 | Missing |
| 2 `ORMap` キー観測除去・値 CRDT 再帰併合 | ORSet(キー集合) + 基底 SPI | ORMap 本体（キー ORSet + 値 map、remove-vs-update 収束、値 put の履歴破壊回避）が未実装 | Missing |
| 3 `ORMultiMap` 多値・空集合キー除去 | ORMap + ORSet | ORMultiMap 本体（`ORMap<K, ORSet<V>>` 合成、空集合キーの可視除去）が未実装 | Missing |
| 4 `LWWMap` キー単位 LWW | ORMap + `LWWRegister<T>` | LWWMap 本体（`ORMap<K, LWWRegister<V>>` 合成、clock 透過）が未実装 | Missing |
| 5 基底契約適合 / delta=full / pruning | 5 基底 SPI（そのまま impl） | `RemovedNodePruning::PruneError` を新型でどう定めるか（整数演算なし） | Constraint |
| 5 Replicator protocol 適合 | `ReplicatorEntry`/`Get`/`Update` は generic | **protocol 変更は不要**（generic に `D: ReplicatedData` を受ける） | Constraint(充足済) |
| 6 no_std / 構造 / イミュータブル | 既存規約・lint | 新規 4 ファイル + sibling test を規約準拠で配置 | Constraint |
| 6 既存 SPI 再利用（基盤型を新設しない） | 全基底 SPI / `VersionVector` / `LWWRegister` | 新規基盤型を作らない方針の徹底 | Constraint |

「Unknown / Research Needed」: §5 に列挙。

## 3. 実装アプローチ（A / B / C）

前提: 4 型はいずれも公開型のため `type-per-file-lint` により**それぞれ独立ファイル必須**。したがって争点は「型の置き場所」ではなく (i) ORSet の dot 基盤をどう用意するか、(ii) ORMap/ORMultiMap/LWWMap を真に合成するか独立実装するか、の 2 軸。

### Option A: VersionVector 拡張 + 薄い合成

- `VersionVector` に `subtract_dots`（観測差分）相当を 1 メソッド追加し、ORSet の dot 基盤に流用。ORMap=キー`ORSet<K>`+値 map、ORMultiMap=`ORMap<K,ORSet<V>>`、LWWMap=`ORMap<K,LWWRegister<V>>` を合成。
- **トレードオフ**: ✅ 因果プリミティブを 1 箇所に集約・最大再利用 ✅ Pekko の `Dot = VersionVector` 層構成に一致 ❌ 既存 `VersionVector` に追加 API（既存型への変更、ただし加算的・後方影響は小）。

### Option B: PNCounterMap パターンで各型自己完結

- 各型が自前の `BTreeMap<UniqueAddress, u64>` dot map と private ヘルパを持ち、`VersionVector` に依存せず PNCounterMap を踏襲。
- **トレードオフ**: ✅ 既存型に一切手を入れない ✅ PNCounterMap と完全に同形でレビュー容易 ❌ dot 差分ロジックが型ごとに重複しやすい ❌ ORMap が ORSet を真に内包せず重複実装になりうる。

### Option C: ハイブリッド（推奨）

- **ORSet を dot 基盤の単一実装**にする（dot 表現は §5-1 で決定）。**ORMap はキーに `ORSet<K>` を内包する真の合成**、ORMultiMap=`ORMap<K, ORSet<V>>`、LWWMap=`ORMap<K, LWWRegister<V>>` として既存 `LWWRegister` を値型に再利用。各型は独立ファイル。
- **トレードオフ**: ✅ Pekko の層構成（ORMap が ORSet を内包）に一致し重複を最小化 ✅ LWWMap/ORMultiMap は薄い合成で済む ✅ 実装順序が依存に沿う（ORSet→ORMap→{ORMultiMap,LWWMap}） ❌ ORSet 完成が後続の前提（直列依存）。
- dot 基盤の置き場所（VersionVector 拡張 or ORSet private）は Option A/B の選択として design で確定。

**推奨**: Option C。ORSet を基盤に据えた合成は Pekko semantics に忠実で、ORMultiMap/LWWMap を最小コストで載せられ、要件2の「集合値は ORMultiMap」契約も層構成で自然に満たす。

## 4. ファイル構造計画（type-per-file 準拠）

| ファイル | 公開型 | sibling test |
|----------|--------|--------------|
| `ddata/or_set.rs` | `ORSet<E>` | `or_set_test.rs` |
| `ddata/or_map.rs` | `ORMap<K, V>` | `or_map_test.rs` |
| `ddata/or_multimap.rs` | `ORMultiMap<K, V>` | `or_multimap_test.rs` |
| `ddata/lww_map.rs` | `LWWMap<K, V>` | `lww_map_test.rs` |

- `ddata.rs` に `mod or_set; … mod lww_map;` と各 `pub use` を最小露出で追加（バレル集約しない）。
- `key.rs` に `ORSetKey<E>` / `ORMapKey<K,V>` / `ORMultiMapKey<K,V>` / `LWWMapKey<K,V>` の type alias を追加（既存の複数 alias 同居前例に倣う。20 行以下の付随物）。
- `type Delta = Self` のため別 Delta 型ファイルは不要。dot 差分ヘルパは §5-1 の決定に従い `VersionVector` メソッド or 各型 private。

## 5. Research Needed（design フェーズへ繰り越す決定事項）

1. **dot 基盤の置き場所**: `VersionVector::subtract_dots`（再利用・causal primitive 集約）を追加するか、ORSet private ヘルパ（既存型不変更）にするか。Pekko の `Dot=VersionVector` に倣うなら前者が自然。
2. **`RemovedNodePruning::PruneError` 戦略**: ORSet/LWWMap は整数演算なしのため `core::convert::Infallible`。ORMap は `type PruneError = V::PruneError` で値型の prune error を伝播するか、統一エラーにするか（trait 境界に影響）。
3. **ORMap の値型安全性**: 値に観測除去集合（ORSet）を `put` で差し替えると因果履歴が壊れる anomaly（Pekko は実行時 `IllegalArgumentException`）。Rust では (a) marker trait による静的排除、(b) doc 契約 + `ORMultiMap` 誘導、のどちらにするか。
4. **ORMap `update`（modify）のシグネチャ**: `update(node, key, initial: V, modify: FnOnce(&V)->V)` か `Option<&V>` ベースか（要件2 AC4 の「初期値は呼び出し元提供」を満たす形）。
5. **ORMultiMap の `withValueDeltas` モード**: Pekko の 2 コンストラクタ（`empty` / `emptyWithValueDeltas`）を両対応するか単一にするか。収束結果は同一で差分効率のみの差のため、初版は単一モードに絞る選択肢あり（design で確定）。
6. **LWWMap の clock API**: 既存 `LWWRegister` の `current_time_millis: i64` + `clock: FnOnce(i64,&T)->i64` を操作シグネチャに透過する形で確定。
7. **property test 基盤**: `cluster-ddata-core-types` のタスクで property test 依存を導入済み。同じ harness（merge の可換・結合・冪等、delta=full、pruning 因果保存）を再利用する。

## 6. 複雑度・リスク

| 単位 | Effort | Risk | 根拠 |
|------|--------|------|------|
| `ORSet` | M (3-7d) | Medium | dot 差分・add-wins 収束・再追加の正当性。後続の基盤で正確性が要 |
| `ORMap` | M (3-7d) | Medium | 値 CRDT 再帰併合、remove-vs-並行 update の収束、値型安全性 |
| `ORMultiMap` | S (1-3d) | Low-Medium | `ORMap<K,ORSet<V>>` 合成 + 空集合キー除去 |
| `LWWMap` | S (1-3d) | Low | `ORMap<K,LWWRegister<V>>` 薄い合成、既存 LWWRegister 再利用 |
| 基底適合 + pruning + property tests | 各型に内包 | Medium | CRDT 則の property 検証を全型で担保 |

- **全体**: M〜L（4 型で約 1〜1.5 週間）。リスクは ORSet の dot/observed-remove 正当性に集中。protocol・既存型への影響は最小（protocol 変更ゼロ、VersionVector への追加は加算的）。
- **Risk 緩和**: ORSet を最初に完成させ property test で収束則を固めてから ORMap 以降を合成で積む。Pekko 実装を semantics の正解として参照。

## 7. design フェーズへの推奨

- **推奨アプローチ**: Option C（ORSet 基盤 + 合成）。
- **design で確定すべき主要決定**: §5 の 1〜6（特に dot 基盤の置き場所、PruneError 戦略、ORMap 値型安全性、ORMultiMap の delta モード範囲）。
- **持ち越す research items**: §5 全項目。protocol 適合は確認済み（変更不要）。
- **境界の再確認**: Replicator runtime / DistributedData extension / DurableStore / typed・std 層は本仕様の対象外（Phase 3）。本仕様は pure CRDT データ型と基底 SPI 適合に限定する。

---

## 8. 設計フェーズの synthesis（2026-06-16）

design 生成時に 3 レンズを適用した結果と、§5 Research Needed の確定。

### 8.1 Generalization
- 4 型は「観測除去 CRDT」の変種。`ORSet` を dot 基盤の一般能力とし、`ORMap` がキー集合に `ORSet` を内包、`ORMultiMap`=`ORMap<K,ORSet<V>>`・`LWWMap`=`ORMap<K,LWWRegister<V>>` を特殊化として薄く載せる（Option C 層化合成）。インターフェースのみ一般化し、実装は現要件に限定。

### 8.2 Build vs. Adopt
- **Adopt**: 既存基底 SPI・`VersionVector`（dot）・`LWWRegister`（LWWMap 値型）・`PNCounterMap` の per-key dot map パターン。
- **Build**: 新規 4 型 + `VersionVector::subtract_dots`（観測差分の因果プリミティブ）。外部 CRDT crate は no_std + 自前 SPI 適合の制約から不採用。

### 8.3 Simplification（採用しないもの）
- `ORMultiMap` の `withValueDeltas` モードは見送り（収束結果は同一、差分効率のみの最適化）。将来の別変更へ。
- ORMap 値型安全性の静的 marker trait は導入しない。`put`（置換）/`update`（merge）の API 分離 + rustdoc 契約 + `ORMultiMap` 誘導で対応（Pekko は実行時例外、本実装は API 分離 + 文書契約）。
- `type Delta = Self` のため Delta 専用ファイルは作らない。dot 差分ヘルパは `VersionVector` に集約し ORSet 内に補助公開型を作らない。

### 8.4 §5 Research Needed の確定
1. **dot 基盤**: `VersionVector::subtract_dots(&self, vvector) -> VersionVector` を新設（Option A）。ORSet は per-element dot = `VersionVector`。
2. **PruneError**: `ORSet`/`ORMultiMap`/`LWWMap` = `core::convert::Infallible`。`ORMap<K,V>` = 条件付き impl（`where V: RemovedNodePruning`）で `type PruneError = V::PruneError`。
3. **ORMap 値型安全性**: `put`/`update` 分離 + 文書契約 + `ORMultiMap` 誘導（静的 marker なし）。
4. **ORMap update**: `update(node, key, initial: V, modify: FnOnce(&V)->V)`（初期値は呼び出し元提供）。
5. **ORMultiMap モード**: 単一モード（`withValueDeltas` 見送り）。
6. **LWWMap clock**: `put(node, key, value, current_time_millis)`（default_clock）+ `put_with_clock(..., clock: FnOnce(i64,&V)->i64)`。
7. **property test**: 既存 `proptest` dev 依存を再利用。

### 8.5 design で判明した追加前提
- **`LWWRegister` の `RemovedNodePruning` 実装が前提**: `LWWMap` の pruning（要件5 AC3）には値型 `LWWRegister<T>` が `RemovedNodePruning` を実装している必要がある（現状未実装）。`lww_register.rs` に追加（`updated_by` が退役ノードなら collapse 先へ置換、`PruneError = Infallible`）。本仕様の変更対象に含める。
- **remove の node 不要**: `ORSet::remove` / `ORMap::remove` は在リポ `PNCounterMap::remove(&self, key)` 慣行に合わせ node 引数なし（観測済み dot を removed 側へ記録）。add 系は dot 採番のため `SelfUniqueAddress` を取る。Pekko の `remove(node, elem)` とは署名が異なるが、観測除去の意味は保たれる。
