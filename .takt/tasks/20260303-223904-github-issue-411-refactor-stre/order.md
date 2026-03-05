## GitHub Issue #411: refactor: streams flow.rs の分割とSource/Flow重複メソッドの共通化

## 背景

`modules/streams/src/core/stage/flow.rs` が **6,359行** あり、プロジェクト最大のファイル。`source.rs`（2,000行）、`sink.rs`（1,174行）も同様のパターンで肥大化している。

## 問題

### 1. flow.rs に3種類のコードが混在（6,359行）

| セクション | 行数 | 内容 |
|-----------|------|------|
| Flow DSLメソッド群 | ~2,210行 | pub fn map/filter/via 等 151本 |
| StreamStage impl（definition関数） | ~1,291行 | *_definition 関数 60本 |
| Logic struct + impl群 | ~2,554行 | 55個のLogic構造体とFlowLogic/GraphStageLogic実装 |

### 2. Source と Flow の73メソッド重複

Pekko では `FlowOps` trait で共通化しているが、Rust にはHKT（Higher-Kinded Types）がないため、`map`, `filter`, `grouped`, `fold` 等73メソッドが `Flow` と `Source` 両方に手書きコピーされている。

各メソッドの構造は以下のパターンがほぼ同一:
```rust
pub fn map<T, F>(self, f: F) -> Flow<In, T, Mat> {
    let (inlet, outlet) = self.graph.add_stage(...);
    self.graph.connect(...);
    Flow::new(self.graph, self.source_outlet, outlet, self.materializer)
}
```

### 3. Logic struct の1ファイル集中

55個の Logic struct（MapLogic, FilterLogic, GroupedLogic...）とその FlowLogic/GraphStageLogic/GraphStage impl がすべて flow.rs 内に定義されており、約2,554行を占める。

## タスク

### Phase 1: Logic struct の分離

- [ ] `flow.rs` 内の55個の Logic struct + impl を `stage/logic/` ディレクトリに分離
- [ ] 各 Logic を独立ファイルに配置（既存の1file1typeルールに従う）
- [ ] flow.rs の *_definition 関数から分離した Logic を参照

### Phase 2: Source/Flow 共通メソッドのマクロ化

- [ ] 73個の共通メソッドのシグネチャと本体パターンを分析
- [ ] 宣言的マクロ（`macro_rules!`）で共通パターンを生成
- [ ] Flow と Source の両方でマクロを展開
- [ ] 戻り型（Flow vs Source）の差異をマクロパラメータで吸収

## 期待効果

- flow.rs: 6,359行 → ~2,200行（Logic分離で~2,500行削減、definition関数は残留）
- source.rs: 2,000行 → ~800行（73メソッドのマクロ共通化）
- 合計: ~4,000行削減見込み
- 新規オペレーター追加時の変更箇所が1箇所に集約

## 優先度

**高**（最大ファイルの分割。コードの見通しと保守性が大幅に改善）

### Labels
refactoring