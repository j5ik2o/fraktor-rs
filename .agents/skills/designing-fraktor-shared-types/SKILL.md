---
name: designing-fraktor-shared-types
description: fraktor-rsの共有型設計を対話的に支援する。&mut self vs &self + 内部可変性の判断、Shared/Handleパターン選択、SharedAccessテンプレート生成を行う。トリガー：「共有型を作りたい」「Sharedパターン」「内部可変性」「&mut selfか&selfか」「Shared型を新設」「SharedLock」「SharedRwLock」「shared design」「共有ラッパー」等の共有型設計リクエスト時に使用。
---

# fraktor-rs 共有型設計

&mut self 原則に基づく共有型の設計・実装を対話的に支援する。

## Workflow

### 1. ヒアリング

対象の型について以下を確認する：

- **型名**: 何を表す型か
- **共有要件**: 複数箇所から参照される必要があるか
- **状態変更**: 可変メソッドが必要か
- **読み書き比率**: 書き込み主体か、読み込み主体か
- **ホットパス**: 高頻度で呼ばれるか

### 2. 判定フロー

```
この型は共有される必要があるか？
├─ No → &mut self で設計（第1選択）→ 完了
└─ Yes → 次へ

状態変更メソッドが必要か？
├─ No → ArcShared<T> で共有（読み取り専用）→ 完了
└─ Yes → 次へ

管理責務（ライフサイクル制御、監視等）があるか？
├─ No → *Shared パターン（薄い同期ラッパー）
└─ Yes → *Handle パターン（管理責務付き）

読み書き比率はどちらが多いか？
├─ 書き込み主体 / 拮抗 → SharedLock<T>
└─ 読み込み主体          → SharedRwLock<T>
```

### 3. テンプレート生成

判定結果に応じて以下のコードを生成する。

#### Shared パターン（書き込み主体: SharedLock）

```rust
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

/// Logic body for Xyz. Mutable operations use &mut self.
pub struct Xyz {
    // state fields
}

impl Xyz {
    /// Creates a new instance.
    pub fn new(/* args */) -> Self {
        Self { /* fields */ }
    }

    /// Mutates internal state (Command).
    pub fn do_something(&mut self, /* args */) -> Result<(), XyzError> {
        // logic
    }

    /// Returns a read-only snapshot (Query).
    pub fn snapshot(&self) -> XyzSnapshot {
        // read-only
    }
}
```

```rust
/// Shared wrapper for Xyz. Interior mutability is confined here.
#[derive(Clone)]
pub struct XyzShared {
    inner: SharedLock<Xyz>,
}

impl XyzShared {
    pub fn new(value: Xyz) -> Self {
        // プロダクション・テスト共通の推奨初期化。
        // DefaultMutex<_> は feature flag に応じて CheckedSpinSyncMutex /
        // StdSyncMutex / SpinSyncMutex に解決される。
        Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(value) }
    }
}

impl SharedAccess<Xyz> for XyzShared {
    fn with_read<R>(&self, f: impl FnOnce(&Xyz) -> R) -> R {
        self.inner.with_read(f)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut Xyz) -> R) -> R {
        self.inner.with_write(f)
    }
}
```

#### Shared パターン（読み込み主体: SharedRwLock）

```rust
use fraktor_utils_core_rs::core::sync::{DefaultRwLock, SharedAccess, SharedRwLock};

#[derive(Clone)]
pub struct XyzShared {
    inner: SharedRwLock<Xyz>,
}

impl XyzShared {
    pub fn new(value: Xyz) -> Self {
        Self { inner: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(value) }
    }
}

impl SharedAccess<Xyz> for XyzShared {
    fn with_read<R>(&self, f: impl FnOnce(&Xyz) -> R) -> R {
        self.inner.with_read(f)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut Xyz) -> R) -> R {
        self.inner.with_write(f)
    }
}
```

#### Handle パターン（管理責務付き）

Shared と同じ構造（`SharedLock<T>` または `SharedRwLock<T>` を内包）だが、
ライフサイクル制御メソッド（start/stop/shutdown 等）を追加する。

### 4. 初期化の標準形

```rust
// 書き込み主体:
SharedLock::new_with_driver::<DefaultMutex<_>>(value)

// 読み込み主体:
SharedRwLock::new_with_driver::<DefaultRwLock<_>>(value)
```

- `DefaultMutex<T>` / `DefaultRwLock<T>` は feature flag に応じてバックエンドを
  自動選択する type alias（`CheckedSpinSync*` / `StdSync*` / `SpinSync*`）。
- **プロダクションコードでもテストコードでも、`DefaultMutex<_>` /
  `DefaultRwLock<_>` を driver として渡すのが標準。** `SpinSyncMutex<_>` /
  `SpinSyncRwLock<_>` を直接指定すると `debug-locks` の re-entry 検知や
  `std-locks` の利点が失われるため、テストでも `DefaultMutex<_>` を使う。

### 5. デッドロック回避チェック

生成したコードに対して以下を確認：

- ロック内で別のロックを取得していないか
- ロック内で EventStream への通知を行っていないか（ロック外に移動すべき）
- ガードやロックを外部に返していないか（`with_read` / `with_write` のクロージャ内で完結すべき）
- with_write 内で長時間のI/Oを行っていないか
- 書き込み主体の経路で `SharedRwLock<T>` を選んでいないか（書き込みロックが頻繁に取られると
  read 側がブロックされて性能が落ちる）

### 6. ファイル配置

- `xyz.rs`: ロジック本体（Xyz）
- `xyz_shared.rs`: 共有ラッパー（XyzShared、`SharedLock<T>` または `SharedRwLock<T>` を内包）
- `xyz/tests.rs`: ロジック本体のテスト（ロックなしで書く）

## 使用例

### 例1: 新しい Registry を共有したい（書き込み主体）

**リクエスト**: 「NameRegistry を複数の ActorCell から参照したい」

**判定**:
- 共有必要? → Yes（複数 ActorCell から参照）
- 状態変更? → Yes（register/unregister）
- 管理責務? → No（単純な登録/検索）
- 読み書き比率? → 書き込み主体（register/unregister と lookup が拮抗）

**結果**: `SharedLock<NameRegistry>` を用いた Shared パターンを適用
- `NameRegistry`: &mut self で register/unregister
- `NameRegistryShared`: `SharedAccess` 実装、`SharedLock::new_with_driver::<DefaultMutex<_>>(...)` で初期化

### 例2: 読み取り専用の Config

**リクエスト**: 「ActorSystemConfig を複数箇所で参照したい」

**判定**:
- 共有必要? → Yes
- 状態変更? → No（設定は不変）

**結果**: `ArcShared<ActorSystemConfig>` で直接共有。Shared 型は不要。

### 例3: 読み込み主体の Snapshot Cache

**リクエスト**: 「ClusterMembershipSnapshot を多数のサブスクライバーから参照、稀に更新」

**判定**:
- 共有必要? → Yes（多数のサブスクライバー）
- 状態変更? → Yes（gossip 受信時に更新）
- 管理責務? → No
- 読み書き比率? → 読み込み主体（更新は稀、参照は多い）

**結果**: `SharedRwLock<ClusterMembershipSnapshot>` を用いた Shared パターンを適用
- `ClusterMembershipSnapshotShared`: `SharedAccess` 実装、`SharedRwLock::new_with_driver::<DefaultRwLock<_>>(...)` で初期化

## 参照ドキュメント

- `.agents/rules/rust/immutability-policy.md`: 判定フローの根拠となる内部可変性ポリシー。ステップ2の判断に迷った場合に参照
- `docs/guides/shared_vs_handle.md`: Shared/Handle パターンの詳細な実装ガイド。テンプレート生成時に既存パターンを確認する際に参照
- `.agents/rules/rust/cqs-principle.md`: Command（&mut self）と Query（&self）の分離原則。テンプレートのメソッド設計時に参照

## 出力ガイドライン

- ロジック本体のテストはロックなしで書くことを推奨
- 既存の類似 Shared 型を `mcp__serena__find_symbol` で検索し、パターンの一貫性を確認
- 初期化は必ず `SharedLock::new_with_driver::<DefaultMutex<_>>(...)` または
  `SharedRwLock::new_with_driver::<DefaultRwLock<_>>(...)` の形にする（テストでも同じ）
