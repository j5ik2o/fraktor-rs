パッケージ構造リファクタリングの **構造品質** を中心にアーキテクチャレビューする。

このレビューの目的は、単に Phase の移設作業が完了したかではなく、最終構造が **高凝集・低結合に前進したか** を判定することにある。

## 必ず読むもの

- `structure-analysis.md`
- `structure-design.md`
- `plan.md`
- `coder-scope.md`
- `coder-decisions.md`
- 変更対象コード

## レビュー観点

### 1. Phase 完了と構造完了を分離して判定する

- Phase 完了:
  - 今回の plan に書かれた移設・削除・ import 更新が終わっているか
- 構造完了:
  - 変更後の構造が、変更理由ごとのまとまりとして理解可能になっているか
  - 次に同種の責務を追加するとき、置き場が一意に予測できるか

Phase 完了でも構造完了でなければ `needs_fix` にする。

### 2. 高凝集

以下を確認する:

- 同一ディレクトリ／モジュール内が単一の変更理由に収束しているか
- `foo / foo_impl / foo_misc` のような責務あいまいな分割が残っていないか
- `core.rs` や親 `mod` が依然として神モジュールになっていないか
- `structure-design.md` で定義したグループごとの CCP が実ファイル配置に反映されているか

### 3. 低結合

以下を確認する:

- 依存方向が `structure-design.md` の依存図どおりに収束しているか
- 三角依存や双方向依存が温存されていないか
- 公開 API 層から private 実装層への逆流がないか
- 単なるファイル移動で、実際の import 集中や責務集中が変わっていない状態でないか

### 4. 過度なフラット配置の解消

以下を確認する:

- root 直下の責務が減ったか、もしくは明確な親グループへ収束したか
- 小さな単機能ディレクトリが同階層に乱立する「横に広いだけ」の構造になっていないか
- `structure-design.md` に記録した before/after の構造メトリクス目標が達成されているか

### 5. future placement predictability

レビューでは必ず 2〜3 個の代表責務について、
「新しく同種の型を追加するならどこへ置くか」を自分で答える。

答えが一意に定まらない場合は構造不良として `needs_fix`。

### 6. 局所最適化の検出

次の状態は **必ず blocking**:

- 今回の移設対象だけは綺麗だが、上位階層の dumping ground が実質そのまま
- 旧責務を別名の新ディレクトリへ移しただけで、変更理由のまとまりが改善していない
- 「private に寄せた」「path shim を消した」だけで、高凝集・低結合の根拠がない
- `coder-decisions.md` に before/after メトリクスがなく、改善量を説明できない

## 検索・照合（必須）

- `#[path = "..."]`、`include!`、旧互換 `pub use` / `pub type` / wrapper mod の有無
- `structure-design.md` の「正準パスと廃止対象」に対する参照残り
- `coder-decisions.md` の構造改善メトリクスと、実コードの整合

## 判定ルール

- **approved**:
  - blocking finding が 0 件
  - Phase 完了だけでなく、構造改善メトリクスでも前進が確認できる
- **needs_fix**:
  - blocking finding が 1 件以上
  - または Phase 完了はしているが構造品質ゲートを満たしていない

## finding の作り方

- `family_tag` は `cohesion` / `coupling` / `discoverability` / `layering` / `migration-integrity` のいずれかを使う
- 問題は「どの原則に反しているか」を明示する
- 修正案は「どこへ寄せるか」「何を削るか」まで具体化する

## やってはいけないこと

- 「今回の Phase の目的は達成しているので approve」とだけ判定すること
- `architect-review.md` を CI 通過確認の代替にすること
- 代表的な配置予測チェックを省略すること
