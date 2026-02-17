Perfect! Now I have completed my analysis. Let me summarize my findings:

## 分析完了

タスク「distinct / distinctByオペレーター（HashSetベースの重複排除フィルタ）を実装する」の分析が完了しました。

### 1. 要件理解

**タスク要件:**
- `distinct`: 全ストリーム履歴に対する重複排除（HashSet/BTreeSetベース）
- `distinctBy`: カスタム抽出関数による重複排除

**既存実装との違い:**
- `drop_repeated`: **連続する重複のみ**をフィルタリング（`Option<Out>` で前回値のみ保持）
- `distinct`: **全履歴の重複**をフィルタリング（`BTreeSet` で全既出値を保持）

**参照資料の確認結果:**
- Gap Analysis (`docs/gap-analysis/streams-gap-analysis.md:121`) に「HashSetベースの重複排除」として記載
- Pekko の FlowOps に相当する機能（参照実装は明示的に指定されていないが、標準的なストリームオペレーター）

### 2. コード調査結果

**既存パターン:**
- フィルタ系オペレーターは `FlowLogic` トレイトを実装
- `filter_definition<In, F>` → `FilterLogic<In, F>` → `FlowLogic::apply` の構造
- `stateful_map` で状態を持つオペレーターを実装可能（`drop_repeated` が使用）

**コレクション制約:**
- `no_std` 環境のため `alloc::collections::BTreeSet` を使用（`HashSet` は std のみ）
- 既存コードで `BTreeMap` を使用している箇所あり (`stream_drive_actor.rs:1`)

**型制約の確認:**
- `drop_repeated` は `Clone + PartialEq` を要求 (`flow.rs:1823`)
- `distinct` は `Clone + Ord` が必要（`BTreeSet` の要件）
- `distinctBy` は `Key: Clone + Ord` が必要

### 3. 影響範囲の特定

**変更が必要なファイル:**

| ファイル | 変更内容 | 理由 |
|---------|---------|------|
| `modules/streams/src/core/stage/flow.rs` | `distinct()`/`distinct_by()` メソッド追加 | Flow オペレーター |
| `modules/streams/src/core/stage/flow.rs` | `distinct_definition()`/`distinct_by_definition()` 関数追加 | 定義関数 |
| `modules/streams/src/core/stage/flow.rs` | `DistinctLogic`/`DistinctByLogic` 構造体追加 | ロジック実装 |
| `modules/streams/src/core/stage/flow.rs` | `FlowLogic` 実装追加 | トレイト実装 |
| `modules/streams/src/core/stage/stage_kind.rs` | `FlowDistinct`, `FlowDistinctBy` 追加 | ステージ種別 |
| `modules/streams/src/core/stage/flow/tests.rs` | テストケース追加 | 品質保証 |

**変更が不要なファイル:**
- `source.rs`: Source は Flow オペレーターを via で利用可能なため追加不要
- `sink.rs`: Sink には該当しない
- その他のモジュール: 影響なし

### 4. 設計方針

**ファイル構成:**
- すべての変更を `flow.rs` に集約（既存パターンに従う）
- テストは `flow/tests.rs` に追加

**実装アプローチ:**

1. **`distinct()` オペレーター:**
   ```rust
   impl<In, Out, Mat> Flow<In, Out, Mat>
   where
     In: Send + Sync + 'static,
     Out: Clone + Ord + Send + Sync + 'static,
   {
     pub fn distinct(mut self) -> Flow<In, Out, Mat> {
       // DistinctLogic を使用
     }
   }
   ```

2. **`distinct_by()` オペレーター:**
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
     {
       // DistinctByLogic を使用
     }
   }
   ```

3. **ロジック実装:**
   ```rust
   struct DistinctLogic<In> {
     seen: BTreeSet<In>,  // 既出値の記録
     _pd: PhantomData<fn(In)>,
   }
   
   struct DistinctByLogic<In, Key, F> {
     seen: BTreeSet<Key>,  // 既出キーの記録
     extract: F,
     _pd: PhantomData<fn(In) -> Key>,
   }
   ```

4. **FlowLogic 実装:**
   - `apply` メソッドで `seen.contains()` チェック
   - 新規値なら `seen.insert()` して出力
   - 既出値なら `Ok(Vec::new())` を返す（フィルタリング）

**型制約の選択根拠:**
- `BTreeSet` を使用するため `Ord` が必要（`PartialEq + Hash` の `HashSet` は std 限定）
- `Clone` は値/キーを Set に保存するために必要

### 5. 実装ガイドライン

**Coder への指示:**

1. `stage_kind.rs` に列挙値を追加:
   - `FlowDistinct` （行19の `FlowFilter` の後）
   - `FlowDistinctBy` （同位置）

2. `flow.rs` の `impl<In, Out, Mat> Flow<In, Out, Mat>` ブロックに:
   - `distinct()` を `drop_repeated()` (行1827) の後に追加
   - `distinct_by()` を `distinct()` の後に追加

3. `flow.rs` の定義関数セクション（行1874以降）に:
   - `distinct_definition()` を追加
   - `distinct_by_definition()` を追加

4. `flow.rs` のロジック構造体セクション（行2855以降）に:
   - `DistinctLogic` を追加
   - `DistinctByLogic` を追加

5. `flow.rs` の `FlowLogic` 実装セクション（行3099以降）に:
   - `impl FlowLogic for DistinctLogic` を追加
   - `impl FlowLogic for DistinctByLogic` を追加

6. `flow/tests.rs` にテストケースを追加:
   - `distinct_removes_duplicates` （基本動作）
   - `distinct_preserves_order` （順序保持）
   - `distinct_by_with_key_function` （カスタムキー）
   - `distinct_by_on_tuples` （タプル型での動作確認）

**注意事項:**
- `use alloc::collections::BTreeSet;` をファイル冒頭に追加
- `Clone` の呼び出しは必要最小限に（`seen.insert(value.clone())` のみ）
- エラーハンドリングは既存パターンに従う（`downcast_value` 使用）

---

**要件ごとの実装必要性判定:**

| 要件 | 必要性 | 根拠 |
|------|--------|------|
| `distinct` オペレーター実装 | **必要** | 現在未実装（`drop_repeated` は連続重複のみフィルタ） |
| `distinct_by` オペレーター実装 | **必要** | 現在未実装 |
| BTreeSet の使用 | **必要** | no_std 制約により HashSet 不可（std 限定）、タスク指示書の「HashSetベース」は概念的な意味 |
| Source への追加 | **不要** | `Source::via(Flow::distinct())` で利用可能（flow.rs:88 の既存パターン） |