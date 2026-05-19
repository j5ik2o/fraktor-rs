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
   └─ Yes → Shared ラッパーパターンを新設（第2選択）

Shared ラッパーパターン:
  inner に SharedLock<A>（書き込み主体）または SharedRwLock<A>（読み込み主体）を
  保持する Shared ラッパー構造体を新設する。
  どちらも utils-core が提供する SharedAccess 準拠の同期ラッパーで、
  ArcShared<dyn ...Backend<T>> を内側に持つ。
  → 詳細は docs/guides/shared_vs_handle.md を参照
```

## ルール

### trait の `&mut self` メソッド

- セマンティクスを重視した設計になっている
- 戻り値を返さないで状態を変えるメソッドは `&self` ではなく `&mut self` が原則
- 安易に `&self` + 内部可変性にリファクタリングしないこと
- **変更する場合は人間から許可を取ること**

### Shared ラッパーパターン（内部可変性の唯一の許容ケース）

`&mut self` メソッドを持つ型 A が複数箇所から共有される場合のみ許容：

```rust
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

// ロジック本体: &mut self
pub struct Xyz { /* state */ }

impl Xyz {
    pub fn do_something(&mut self, arg: T) -> Result<()> { /* logic */ }
    pub fn snapshot(&self) -> Snapshot { /* read-only */ }
}

// 共有ラッパー: 内部可変性はここだけ
#[derive(Clone)]
pub struct XyzShared {
    inner: SharedLock<Xyz>,        // 書き込み主体なら SharedLock<Xyz>
    // inner: SharedRwLock<Xyz>,   // 読み込み主体なら SharedRwLock<Xyz>
}

impl XyzShared {
    pub fn new(value: Xyz) -> Self {
        // プロダクション・テスト共通の推奨初期化:
        //   DefaultMutex<_> を driver として渡す。feature flag に応じて
        //   CheckedSpinSyncMutex / StdSyncMutex / SpinSyncMutex に解決される。
        Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(value) }
    }
}
```

`SharedLock<T>` / `SharedRwLock<T>` は `utils-core::core::sync` が提供し、
`SharedAccess<T>` を実装する。`with_read` / `with_write` を介してロック区間内で
クロージャを実行する形に API を絞る。

### SharedLock と SharedRwLock の使い分け

| 同期ラッパー | 用途 |
|--------------|------|
| `SharedLock<T>` | 書き込み主体、または読み書き比率が拮抗 |
| `SharedRwLock<T>` | 読み込み主体（書き込みは稀、参照は多い） |

迷ったら `SharedLock<T>` を選ぶ。`SharedRwLock<T>` への切替は実測でホットパス
が読み込み主体だと判明した時点で行う。

### 初期化の標準形（プロダクション・テスト共通）

```rust
// 書き込み主体:
SharedLock::new_with_driver::<DefaultMutex<_>>(value)

// 読み込み主体:
SharedRwLock::new_with_driver::<DefaultRwLock<_>>(value)
```

- `DefaultMutex<T>` / `DefaultRwLock<T>` は feature flag によって
  `CheckedSpinSync*` / `StdSync*` / `SpinSync*` に解決される type alias。
- **プロダクションコードでもテストコードでも、`DefaultMutex<_>` / `DefaultRwLock<_>`
  を driver として渡すのが標準。** `SpinSyncMutex<_>` / `SpinSyncRwLock<_>` を直接
  指定するとテスト時の re-entry 検知（`debug-locks` feature）や std backend の利点
  が失われるため、テストでも `DefaultMutex<_>` を使うことを推奨する。

### 命名

- 薄い同期ラッパー → `*Shared`
- ライフサイクル/管理責務 → `*Handle`
- 所有権一意・同期不要 → サフィックスなし

## 禁止パターン

- 既存の `&mut self` trait メソッドを `&self` + 内部可変性に変更（人間許可なし）
- 共有不要な型を `SharedLock<T>` / `SharedRwLock<T>` でラップ
- Shared ラッパーパターン適用時に元のロジック型を削除
- ガードやロックを外部に返す（ロック区間は `with_read` / `with_write` のクロージャ内に閉じる）
- `ArcShared<SpinSyncMutex<T>>` のような手書きラッパーを新規作成（`SharedLock<T>` を使うこと）
- `ArcShared<SpinSyncRwLock<T>>` のような手書きラッパーを新規作成（`SharedRwLock<T>` を使うこと）
