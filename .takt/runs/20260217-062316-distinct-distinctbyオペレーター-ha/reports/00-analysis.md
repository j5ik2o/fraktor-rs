# タスク計画

## 元の要求
distinct / distinctByオペレーター（HashSetベースの重複排除フィルタ）を実装する

## 分析結果

### 目的
fraktor-rs streams モジュールに、ストリーム全体の履歴に対する重複排除を行う `distinct` および `distinct_by` オペレーターを実装する。既存の `drop_repeated` は連続する重複のみを除去するが、`distinct` は全履歴に対して重複チェックを行う。

### スコープ

**変更対象ファイル:**
- `modules/streams/src/core/stage/flow.rs` - オペレーター本体、定義関数、ロジック実装
- `modules/streams/src/core/stage/stage_kind.rs` - ステージ種別の列挙値追加
- `modules/streams/src/core/stage/flow/tests.rs` - テストケース追加

**変更不要（根拠あり）:**
- `modules/streams/src/core/stage/source.rs` - Source は `via(Flow::distinct())` で利用可能（flow.rs:88 の既存パターン）
- `modules/streams/src/core/stage/sink.rs` - Sink には該当しない
- その他のモジュール - 影響なし

**影響範囲:**
- Flow オペレーター API に2つのメソッド追加
- no_std 環境での動作保証（alloc::collections::BTreeSet 使用）

### 設計判断

#### ファイル構成
| ファイル | 役割 |
|---------|------|
| `modules/streams/src/core/stage/stage_kind.rs` | `FlowDistinct`, `FlowDistinctBy` 列挙値追加（行19付近） |
| `modules/streams/src/core/stage/flow.rs` | オペレーターメソッド（行1827以降）、定義関数（行1874以降）、ロジック構造体（行2855以降）、FlowLogic実装（行3099以降） |
| `modules/streams/src/core/stage/flow/tests.rs` | テストケース追加 |

#### 設計パターン
- **Filter + Stateful パターン**: `FilterLogic` と同様の構造で、内部状態として `BTreeSet` を保持
- **既存パターン準拠**: `filter_definition` (flow.rs:2009) と同じ定義関数構造
- **型制約による分離**: `distinct` は `Clone + Ord`、`distinct_by` は `Key: Clone + Ord` で別 impl ブロック

#### コレクション選択
**`BTreeSet` を使用する理由:**
- no_std 環境で利用可能（`alloc::collections` に含まれる）
- `HashSet` は std 限定のため使用不可
- タスク指示書の「HashSetベース」は概念的な重複排除を指す
- 既存コードで `BTreeMap` を使用（stream_drive_actor.rs:1）

#### API設計

**`distinct()` メソッド:**
```rust
impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Clone + Ord + Send + Sync + 'static,
{
  pub fn distinct(mut self) -> Flow<In, Out, Mat>
}
```
- 型制約: `Clone + Ord` （BTreeSet の要件）
- 位置: `drop_repeated()` (flow.rs:1827) の後

**`distinct_by()` メソッド:**
```rust
impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  pub fn distinct_by<Key, F>(mut self, extract: F) -> Flow<In, Out, Mat>
  where
    Key: Clone + Ord + Send + Sync + 'static,
    F: FnMut(&Out) -> Key + Send + Sync + 'static,
}
```
- キー抽出関数 `F` を受け取る
- `Out` に `Ord` を要求しない（キーのみに要求）

#### ロジック実装

**`DistinctLogic` 構造体:**
```rust
struct DistinctLogic<In> {
  seen: BTreeSet<In>,
  _pd: PhantomData<fn(In)>,
}
```
- `seen` に既出値を記録
- `FlowLogic::apply` で `contains` チェック → `insert` → 出力判定

**`DistinctByLogic` 構造体:**
```rust
struct DistinctByLogic<In, Key, F> {
  seen: BTreeSet<Key>,
  extract: F,
  _pd: PhantomData<fn(In) -> Key>,
}
```
- `extract` 関数でキーを抽出
- キーのみを `seen` に保存（メモリ効率）

### 実装アプローチ

**ステップ1: ステージ種別追加**
- `stage_kind.rs` の `StageKind` enum に `FlowDistinct`, `FlowDistinctBy` を追加（行19付近、`FlowFilter` の後）

**ステップ2: オペレーターメソッド追加**
- `flow.rs` の `impl<In, Out, Mat> Flow<In, Out, Mat>` ブロックに `distinct()` を追加（行1827以降、`drop_repeated()` の後）
- 別の `impl` ブロック（`Out: Clone + Ord` 制約なし）に `distinct_by()` を追加

**ステップ3: 定義関数実装**
- `distinct_definition<In>()` を追加（行1874以降、他の定義関数と同じ構造）
- `distinct_by_definition<In, Key, F>()` を追加

**ステップ4: ロジック構造体実装**
- `DistinctLogic<In>` を追加（行2855以降、他のロジック構造体と並べる）
- `DistinctByLogic<In, Key, F>` を追加
- `use alloc::collections::BTreeSet;` をファイル冒頭（行1）に追加

**ステップ5: FlowLogic トレイト実装**
- `impl FlowLogic for DistinctLogic<In>` を追加（行3099以降）
  - `apply` メソッド: `downcast_value` → `contains` チェック → `insert` → `Vec::new()` or `vec![value]`
- `impl FlowLogic for DistinctByLogic<In, Key, F>` を追加
  - `apply` メソッド: `downcast_value` → `extract` → `contains` チェック → `insert` → 出力判定

**ステップ6: テスト実装**
- `flow/tests.rs` に以下を追加:
  - `distinct_removes_duplicates`: `[1,2,1,3,2,4]` → `[1,2,3,4]`
  - `distinct_preserves_order`: 最初の出現順序を保持
  - `distinct_by_with_key_function`: `distinct_by(|x| x % 10)` の動作確認
  - `distinct_by_on_tuples`: タプルの一部をキーにする例

## 実装ガイドライン

**コーディング規約:**
- 既存の `filter` (flow.rs:2009), `drop` (flow.rs:2029) と同じ構造を踏襲
- エラーハンドリングは `downcast_value` を使用
- `Clone` 呼び出しは `seen.insert(value.clone())` のみ（パフォーマンス考慮）
- ドキュメントコメントは既存オペレーターと同じフォーマット

**メモリ考慮事項:**
- `BTreeSet` は無制限に成長するため、長時間実行されるストリームでは注意が必要
- ドキュメントに「メモリ使用量は既出要素数に比例する」旨を記載

**テストデータ設計:**
- 数値型（`u32`）で基本動作を確認
- タプル型 `(String, u32)` で `distinct_by` の実用例を示す
- 空ストリーム、単一要素、全要素重複のエッジケースをカバー

**エラーケース:**
- `downcast_value` 失敗時は既存パターンに従い `StreamError` を返す
- `BTreeSet` の操作は infallible のためエラーハンドリング不要

**パフォーマンス:**
- `BTreeSet::contains`: O(log n)
- `BTreeSet::insert`: O(log n)
- 重複が多い場合は効果的、重複が少ない場合はオーバーヘッド

## 確認事項
なし（要件は明確、実装パターンは既存コードで確立済み）