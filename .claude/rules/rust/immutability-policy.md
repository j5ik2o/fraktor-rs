# fraktor-rs 内部可変性ポリシー

## 原則

**内部可変性をデフォルトでは禁止する。可変操作はまず `&mut self` で設計すること。**

`&self` メソッド + 内部可変性を安易に使うと Rust の借用システムの価値が失われる。

## 判定フロー

```
1. この型は共有される必要があるか？
   ├─ No → &mut self で設計（第1選択）
   └─ Yes → 次へ

2. 状態変更メソッドが必要か？
   ├─ No → ArcShared<T> で共有（読み取り専用）
   └─ Yes → AShared パターンを新設（第2選択）

AShared パターン:
  inner に ArcShared<ToolboxMutex<A, TB>> を保持する AShared 構造体を新設
  → 詳細は docs/guides/shared_vs_handle.md を参照
```

## ルール

### trait の `&mut self` メソッド

- セマンティクスを重視した設計になっている
- 戻り値を返さないで状態を変えるメソッドは `&self` ではなく `&mut self` が原則
- 安易に `&self` + 内部可変性にリファクタリングしないこと
- **変更する場合は人間から許可を取ること**

### AShared パターン（内部可変性の唯一の許容ケース）

`&mut self` メソッドを持つ型 A が複数箇所から共有される場合のみ許容：

```rust
// ロジック本体: &mut self
pub struct XyzGeneric<TB: RuntimeToolbox> { /* state */ }

impl<TB: RuntimeToolbox> XyzGeneric<TB> {
    pub fn do_something(&mut self, arg: T) -> Result<()> { /* logic */ }
    pub fn snapshot(&self) -> Snapshot { /* read-only */ }
}

// 共有ラッパー: 内部可変性はここだけ
#[derive(Clone)]
pub struct XyzSharedGeneric<TB: RuntimeToolbox> {
    inner: ArcShared<ToolboxMutex<XyzGeneric<TB>, TB>>,
}
```

### 命名

- 薄い同期ラッパー → `*Shared`
- ライフサイクル/管理責務 → `*Handle`
- 所有権一意・同期不要 → サフィックスなし

## 禁止パターン

- 既存の `&mut self` trait メソッドを `&self` + 内部可変性に変更（人間許可なし）
- 共有不要な型に `ArcShared<ToolboxMutex<T>>` を使用
- `AShared` パターン適用時に元の型を削除
- ガードやロックを外部に返す（ロック区間はメソッド内に閉じる）
