## Context

`mailbox-once-cell` (#1570) で Mailbox の write-once フィールドを `spin::Once<T>` に置換し、mailbox enqueue 21% 高速化を実証した。同じパターンを他の `*Shared` 型に適用する。

## Goals / Non-Goals

**Goals:**
- write-once パターンの `SharedLock<T>` / `SharedRwLock<T>` を `spin::Once<T>` に置換
- ベンチマークで効果を計測

**Non-Goals:**
- single-thread-access パターンの `RefCell` 化
- Mailbox `user_queue_lock` の削減
- actor-core 以外のクレート（stream-core, cluster-core 等）の最適化

## Decisions

### 1. 各候補の置換判定基準

write-once と判定する条件:
- フィールドが初期化後に `with_lock(|s| *s = value)` や `call_once` で **1 回だけ**セットされる
- 以後のアクセスが全て `with_read(|s| ...)` または `get()` である
- セット後の値の変更（replace, take, swap）がない

各候補はコード読解で上記を検証してから置換する。検証が不合格なら候補から除外する。

### 2. 置換パターン

```rust
// Before
pub struct FooShared {
  inner: SharedLock<Box<dyn Foo>>,
}

impl FooShared {
  pub fn new(foo: Box<dyn Foo>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(foo) }
  }
  pub fn with_read<R>(&self, f: impl FnOnce(&dyn Foo) -> R) -> R {
    self.inner.with_read(|inner| f(&**inner))
  }
}

// After
pub struct FooShared {
  inner: spin::Once<Box<dyn Foo>>,
}

impl FooShared {
  pub fn new(foo: Box<dyn Foo>) -> Self {
    let s = Self { inner: spin::Once::new() };
    s.inner.call_once(|| foo);
    s
  }
  pub fn with_read<R>(&self, f: impl FnOnce(&dyn Foo) -> R) -> R {
    if let Some(inner) = self.inner.get() {
      f(&**inner)
    } else {
      panic!("FooShared not initialized");
    }
  }
}
```

### 3. `spin::Once::initialized()` による即時初期化

`spin::Once` は `const fn initialized(data: T) -> Self` を提供する。コンストラクタで値が確定している場合はこれを使い、`call_once` のオーバーヘッドも省略できる:

```rust
pub fn new(foo: Box<dyn Foo>) -> Self {
  Self { inner: spin::Once::initialized(foo) }
}
```

## Risks / Trade-offs

- [Risk] write-once だと判断した箇所が実は再セットされるケース → Mitigation: 各候補をコード読解で検証。`spin::Once::call_once` は 2 回目の呼び出しを無視するため、panic はしないが値が更新されない
- [Risk] `spin::Once::get()` が `None` を返すケース（初期化前アクセス） → Mitigation: 初期化順序が保証されている箇所のみ対象。不安な場合は `expect("not initialized")` で fail-fast

## Open Questions

- `MiddlewareShared` が動的 middleware 追加/削除をサポートする必要があるか → 現状のコードでは初期化後の変更なし。将来的に hot reload が必要になったら `SharedRwLock` に戻す
