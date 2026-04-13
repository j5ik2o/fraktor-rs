# fraktor-rs CQS 原則

## 原則

**CQS (Command-Query Separation) をできるだけ守ること。**

- **Query**: 状態を読み取る（`&self`、戻り値あり）
- **Command**: 状態を変更する（`&mut self`、戻り値なし or `Result<(), E>`）

## 判定フロー

```
1. このメソッドは状態を変更するか？
   ├─ No → &self + 戻り値（Query）
   └─ Yes → 次へ

2. 戻り値が必要か？
   ├─ No → &mut self + () または Result<(), E>（Command）
   └─ Yes → 次へ

3. CQS 違反なしでロジックが書けるか？
   ├─ Yes → 2つのメソッドに分離
   └─ No → 人間の許可を得て CQS 違反を許容
```

## 許容される違反（人間許可前提）

| ケース | 理由 |
|--------|------|
| `Vec::pop` 相当 | 読み取りだが更新が不可避 |
| `Iterator::next` | プロトコル上 `&mut self` + `Option<T>` が必要 |
| Builder パターン | メソッドチェーンのため `&mut self` を返す |

## コード例

```rust
// ❌ WRONG: CQS 違反（状態変更 + 値返却）
fn process_and_get(&mut self) -> ProcessedData {
    self.state += 1;
    ProcessedData::new(self.state)
}

// ✅ CORRECT: 分離
fn process(&mut self) {
    self.state += 1;
}
fn processed_data(&self) -> ProcessedData {
    ProcessedData::new(self.state)
}

// ✅ ACCEPTABLE: Vec::pop 相当（人間の許可前提）
// NOTE: ロジック上分離不可のため CQS 違反を許容
fn pop_item(&mut self) -> Option<Item> {
    self.items.pop()
}
```

## 禁止パターン

- `&mut self` + 戻り値を安易に使用
- 「便利だから」という理由で CQS 違反
- 内部可変性で `&self` + 戻り値に変更して CQS 違反を隠蔽
