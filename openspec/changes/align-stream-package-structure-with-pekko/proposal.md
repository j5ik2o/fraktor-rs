## Why

`modules/stream/src/core` は現在、`graph`、`stage`、`mat`、`shape`、`queue`、`buffer`、`restart` などの責務が fraktor 都合で分割されている一方、Pekko 側の `org/apache/pekko/stream` は root、`scaladsl`、`javadsl`、`stage`、`impl`、`impl/fusing`、`impl/io`、`serialization`、`snapshot` といった責務境界で整理されている。現状の構造では Pekko の参照箇所を追うたびに fraktor 側の対応位置がぶれ、実装移植・比較・レビューのコストが高い。

正式リリース前で後方互換性を要求しない今の段階で、`modules/stream` の package 構造そのものを Pekko に対応付けやすい形へ寄せるべきである。機能ギャップの解消より先に構造の対応表を安定させることで、今後の operator 追加や内部改善の着地点を明確にできる。

基本方針は Pekko 互換であり、評価基準は「Pekko互換以上なら採用する」である。Pekko より曖昧になる独自化は避ける一方、Rust 側でより高い凝集性・探索性・責務明確化を実現できる場合は、Pekko の package 名から意図的に外れても採用する。

## What Changes

- `modules/stream/src/core` の最上位 package を Pekko の責務境界に対応付けやすい形へ再編する
- DSL 公開面の `Source`、`Flow`、`Sink`、`BidiFlow`、`*WithContext`、subflow 群を、現在の `stage` 中心配置から Pekko の DSL 境界に対応する package へ整理する
- 実行基盤の `graph`、`graph_interpreter`、`boundary_*`、`stage/flow/logic/*` を、Pekko の `impl` / `impl/fusing` に対応する内部 package へ整理する
- materialization 関連の `mat/*` と lifecycle / keep / completion の責務境界を、Pekko の root materializer 語彙と内部実装に分離して整理する
- `buffer`、`queue`、`hub`、`restart`、`framing`、`json_framing`、`compression`、`stream_converters`、`file_io` の置き場所を Pekko の `root` / `impl` / `impl/io` 対応で見直す
- `attributes`、root queue/result types、root restart settings、`impl/queue`、`impl/hub`、`impl/materialization` まで含めて一括で整合した package 再編を完了させる
- `core` と `std` の層分離は維持しつつ、Pekko の package を Rust にそのまま複製するのではなく、fraktor の `core` / `std` 境界の中で対応付ける
- `shape/` は Pekko 非同型だが、Rust 側での型凝集と探索性を高める改善提案として維持する
- **BREAKING** `crate::core::*`、`crate::core::stage::*`、`crate::std::*` の一部 import path を新しい package 経由へ変更する
- 実装時は file move / mod wiring 単位で `./scripts/ci-check.sh ai dylint` を実行し、最終的に `./scripts/ci-check.sh ai all` で全体確認する

## Capabilities

### New Capabilities

- `stream-package-structure`: stream モジュールの package 構造を Pekko 対応の責務境界へ再編する

### Modified Capabilities

## Impact

- 影響対象コード: `modules/stream/src/core.rs`、`modules/stream/src/core/**`、`modules/stream/src/std/**`、`modules/stream/tests/**`
- 影響対象 API: `crate::core` 配下の module path、`Source` / `Flow` / `Sink` 系 DSL の import path、内部 `pub(crate)` module path
- 依存関係への影響: 依存 crate の追加は不要。`mod` 配線、`use` 文、tests/examples の import 更新が中心
- 検証への影響: 構造変更のたびに `./scripts/ci-check.sh ai dylint`、最終的に `./scripts/ci-check.sh ai all` が必要
