# 調査・設計判断

## 要約

- **機能**: `actor-cell-facet-structure`
- **ディスカバリー範囲**: 拡張（既存 actor-core-kernel の内部構造リファクタリング）
- **主要な発見**:
  - `docs/gap-analysis/actor-gap-analysis.md` は `ActorCell` の dungeon facet 未分離を medium/high の構造ギャップとして記録している。
  - 現在の `actor_cell.rs` は 1,809 行、`actor_cell_test.rs` は 2,389 行で、dispatch / fault handling / death watch / children / receive timeout に加えて stash / timers / pipe / adapter handle が同居している。
  - `ActorCell` を public type のまま維持し、`actor_cell.rs` 配下の private submodule に同一型 `impl ActorCell` を分割すれば、public API を増やさず責務境界を作れる。

## 調査ログ

### ActorCell 構造ギャップ

- **背景**: feature slug が `actor-cell-facet-structure` であり、既存 gap analysis に同名の構造課題がある。
- **参照した情報源**:
  - `docs/gap-analysis/actor-gap-analysis.md`
  - `modules/actor-core-kernel/src/actor/actor_cell.rs`
  - `modules/actor-core-kernel/src/actor/actor_cell_test.rs`
  - `modules/actor-core-kernel/src/actor/actor_cell_state.rs`
- **発見**:
  - gap analysis は Pekko 側の `actor/dungeon/` 分離を根拠に、fraktor-rs の `ActorCell` が複数責務を単一ファイルに抱えていることを指摘している。
  - `actor_cell.rs` には生成・dispatcher 接続、child registry、death watch、stash、timer、pipe task、fault handling、lifecycle、message invoker が混在している。
  - `actor_cell_state.rs` は状態保持を一部切り出しているが、状態を操作する振る舞いは root file に集中している。
- **含意**:
  - 新しい runtime feature ではなく、既存動作を保持する内部構造変更として扱う。
  - `SystemState` 分割や public surface 棚卸しは別の構造ギャップなので、本仕様には含めない。

### 参照実装の扱い

- **背景**: project rule は Pekko / protoactor-go の参照を求める。
- **参照した情報源**:
  - `references/pekko`
  - `references/protoactor-go`
  - `docs/gap-analysis/actor-gap-analysis.md`
- **発見**:
  - この worktree の `references/pekko` / `references/protoactor-go` 配下には該当ファイル実体が存在しない状態だった。
  - gap analysis は既に Pekko の `actor/dungeon/` と `ActorCell.scala` 行数を比較根拠として記録している。
- **含意**:
  - 本仕様では live reference tree ではなく、repo-local の gap analysis を参照実装比較の evidence として採用する。
  - 実装時に reference submodule が復元されている場合は、`Dispatch` / `FaultHandling` / `DeathWatch` / `Children` / `ReceiveTimeout` の分割と命名を再確認する。

### 既存 Rust 構造ルール

- **背景**: fraktor-rs は custom dylint によって module layout と test placement を強く制約している。
- **参照した情報源**:
  - `.agents/rules/rust/index.md`
  - `.agents/rules/rust/type-organization.md`
  - `.agents/rules/rust/immutability-policy.md`
  - `modules/actor-core-kernel/src/actor.rs`
- **発見**:
  - `mod.rs` は禁止で、`actor_cell.rs` + `actor_cell/*.rs` の階層は既存規約に合う。
  - 公開型は増やさない方針なら type-per-file-lint の追加負荷を避けられる。
  - sibling test は対象ファイルごとに分割でき、root `actor_cell_test.rs` は生成・配線の回帰に縮小できる。
- **含意**:
  - facet は private submodule と同一型 `impl ActorCell` で表現する。
  - 新しい public trait による `ActorCell: ChildrenFacet + FaultHandlingFacet` のような分割は採用しない。

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|------|------|--------------|------|
| private submodule への `impl ActorCell` 分割 | `actor_cell.rs` に struct と wiring を残し、`actor_cell/*.rs` に責務別 impl を置く | public API を増やさず、既存呼び出し元の変更を最小化できる | private field への依存は残るため、facet 境界は module boundary で示す | 採用 |
| public/private trait facet | `ChildrenFacet` 等の trait を作り `ActorCell` に実装する | 境界名が型に現れる | trait が単一実装の薄い抽象になり、public surface や dispatch 境界を増やしやすい | 不採用 |
| 新しい delegate state/handler 型 | children/fault/death-watch handler 型へ状態操作を委譲する | 単体化しやすい | shared state 受け渡しが増え、TOCTOU と内部可変性の再設計に広がる | 今回は不採用 |

## 設計判断

### 判断: ActorCell は同一型のまま private facet module に分割する

- **背景**: 要件は構造改善であり、利用者向け contract の拡大ではない。
- **検討した代替案**:
  1. private submodule 分割 — 同一型 `impl ActorCell` を責務別ファイルへ移動する。
  2. trait facet 分割 — trait を増やして境界を型として表す。
  3. delegate handler 型 — 各責務を独立型へ移す。
- **採用したアプローチ**: private submodule 分割。
- **根拠**: 既存 call site、public API、state ownership を変えず、巨大ファイル問題だけを先に解消できる。
- **トレードオフ**: facet 間の private field access は残る。完全な ownership split は後続で必要になった場合に設計する。
- **フォローアップ**: 実装後、`actor_cell.rs` が root orchestration へ縮小され、各 facet test が対象責務を担っていることを検証する。

### 判断: SystemState と public surface 棚卸しは境界外に置く

- **背景**: gap analysis には `SystemState` 分割と public surface の広さも記録されている。
- **検討した代替案**:
  1. 同時に SystemState を分割する。
  2. 同時に ActorCell public re-export を縮小する。
  3. ActorCell facet 分割に限定する。
- **採用したアプローチ**: ActorCell facet 分割に限定する。
- **根拠**: SystemState と public surface は別の依存・互換リスクを持ち、同時に扱うと review scope が肥大化する。
- **トレードオフ**: gap analysis の構造課題すべては解消しない。
- **フォローアップ**: ActorCell 分割完了後に SystemState 分割を別 spec として扱う。

## リスクと緩和策

- **リスク**: メソッド移動中に visibility や module path が壊れる。
  **緩和策**: private submodule で `use super::ActorCell` を使い、public symbol を増やさない。
- **リスク**: テスト移動で回帰シナリオが失われる。
  **緩和策**: 既存テストを facet 対応で分類し、削除ではなく移動・縮小を基本にする。
- **リスク**: 単純なファイル分割だけで、root file が再び肥大化する。
  **緩和策**: `actor_cell.rs` の責務を struct/create/accessor/module wiring に限定する。

## 参考資料

- `docs/gap-analysis/actor-gap-analysis.md` — ActorCell dungeon facet 未分離の比較根拠。
- `.agents/rules/rust/index.md` — Rust module layout / test placement / no_std / lint 制約。
- `docs/guides/lock_free_design.md` — ActorCell の排他制御と SharedAccess 方針。
