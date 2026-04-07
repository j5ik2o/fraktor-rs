# fraktor-rs Mutex 抽象レイヤの問題分析 — 別 AI 説明用サマリ

## プロジェクト前提

- **対象**: `fraktor-rs` (Rust アクターフレームワーク, Apache Pekko / protoactor-go を参照実装)
- **リポジトリ**: `j5ik2o/fraktor-rs` (作業はリポジトリルートを基準にした相対パスで記述する)
- **設計価値観**: YAGNI, Less is more, 後方互換不要(リリース前開発フェーズ)
- **制約**: `modules/*/src/core/` は `no_std`、`modules/*/src/std/` がアダプタ
- **規約場所**: `.agents/rules/rust/*.md`

## 発端

AI 生成コードによるデッドロック(Mutex 再入)を機械的に防ぐため、**debug ビルド限定で再入を panic 検出する `DebugMutex`** を導入したい。release では無効化。

ところが調査過程で、ロック抽象レイヤ自体に深刻な過剰設計とデッドコードが存在することが判明した。

---

## 現状の型階層

```
# utils crate 定義
pub type RuntimeMutex<T>  = RuntimeMutexBackend<T>   # modules/utils/src/core/sync/runtime_lock_alias.rs
pub type NoStdMutex<T>    = RuntimeMutex<T>          # 同ファイル
pub type RuntimeRwLock<T> = RuntimeRwLockBackend<T>  # 同ファイル

# modules/utils/src/lib.rs
#[cfg(feature = "std")]
pub(crate) type RuntimeMutexBackend<T> = StdSyncMutex<T>   # std::sync::Mutex ラッパー
#[cfg(not(feature = "std"))]
pub(crate) type RuntimeMutexBackend<T> = SpinSyncMutex<T>  # spin::Mutex ラッパー

# trait
pub trait SyncMutexLike<T> { ... }   # modules/utils/src/core/sync/sync_mutex_like.rs
pub trait SyncRwLockLike<T> { ... }  # modules/utils/src/core/sync/sync_rwlock_like.rs
```

## 具体型の定義場所

| 型 | パス |
|----|------|
| `SpinSyncMutex<T>` | `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs` |
| `StdSyncMutex<T>` | `modules/utils/src/std/sync_mutex.rs` |
| `SpinSyncRwLock<T>` | `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` |
| `StdSyncRwLock<T>` | `modules/utils/src/std/sync_rwlock.rs` |

---

## 発見された問題

### 問題 1: 抽象レイヤの漏洩(`SpinSyncMutex` 直接参照が 44 ファイル)

`RuntimeMutex` という抽象を用意しておきながら、その裏の具体型 `SpinSyncMutex` を直接 import しているファイルが **44 ファイル** 存在する。

**内訳**:
- `stream-core` 本体+テスト: 21 ファイル(最大の漏洩源)
- `actor-core` テスト・manual driver: 3 ファイル
- `remote-adaptor-std`: 1 ファイル
- `utils` 内部(queue モジュール): 6 ファイル
- その他(utils 自身の定義・テスト): 13 ファイル

**使い方はすべて単純**: `ArcShared<SpinSyncMutex<T>>` または `SpinSyncMutex<Option<T>>` のみ。`as_inner()` 等の固有 API 依存はなし。

### 問題 2: `SyncQueueShared` 系 8 ファイルは完全なデッドコード

**場所**: `modules/utils/src/core/collections/queue/`

**ファイル一覧**:
- `sync_queue_shared.rs`
- `sync_mpsc_producer_shared.rs`
- `sync_mpsc_consumer_shared.rs`
- `sync_spsc_producer_shared.rs`
- `sync_spsc_consumer_shared.rs`
- `sync_spsc_producer_shared/tests.rs`
- `tests.rs`
- `queue.rs` の `pub use` エントリ

**状態**:
- `queue.rs` で `pub use` 公開 API 扱い
- **workspace 全体で利用者ゼロ**(utils 内部テスト除く)
- `actor-core` のメールボックスは直接 `RuntimeMutex` + 独自実装で書かれており、この抽象を使っていない

**推測**: メールボックス用に作ったが、別の方法で実装された結果残った "forked overengineering"。

**致命的な副作用**: `SyncMutexLike` trait のジェネリック境界としての唯一の実使用箇所がこの 8 ファイル。削除すると trait の存在意義がゼロになる。
```rust
// modules/utils/src/core/collections/queue/sync_queue_shared.rs:26
pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>
where M: SyncMutexLike<SyncQueue<T, K, B>>, { ... }
```
しかも **`M` に `SpinSyncMutex` 以外を渡している呼び出し元は存在しない**(workspace 全域 grep 済み)。ジェネリクス自体が実質死んでいる。

### 問題 3: `RuntimeMutex` 抽象は過剰設計

`RuntimeMutex` は `feature = "std"` で `StdSyncMutex` と `SpinSyncMutex` を切り替えるためにある。しかし:

1. **プロダクションで `StdSyncMutex` 固有挙動(poisoning / OS parking)に依存している実コードはゼロ**
  - `StdSyncMutex` の grep 結果 6 ファイルはすべて `utils/src/std/sync_mutex*` の定義と自前テスト
2. **`spin::Mutex` は `portable-atomic + critical-section` で no_std/std 両対応済み**
  - `modules/utils/Cargo.toml`: `spin = { ..., features = ["mutex", "spin_mutex", "rwlock", "portable_atomic"] }`
  - 全環境で動くので切り替えの必要自体がない
3. **アクターフレームワークは short-held lock 前提**
  - OS parking より spin の方が実質的に有利
4. **二重の型エイリアス間接層**
  - `RuntimeMutex` → `RuntimeMutexBackend` → `SpinSyncMutex`/`StdSyncMutex`
  - 1層は完全に無意味

### 問題 4: `SyncMutexLike` / `SyncRwLockLike` trait は幽霊化

- `SyncMutexLike` の **ジェネリック境界としての使用** は `SyncQueueShared` 系 8 ファイルのみ
- 問題 2 のデッドコードを削除すると、**trait の実ユーザーが消える**
- 残るのは `SyncRwLockLike::write()` 等を呼ぶためだけの inherent method 相当の使い方
  - `actor-core` の `*_shared.rs` で `use ...sync_rwlock_like::SyncRwLockLike;` して `.write()` を呼んでいる
  - これは trait を inherent method の代わりに使っているだけで、ジェネリックとしての価値はない
- 1 実装しかない trait は YAGNI 違反

### 問題 5: 規約と抽象の不整合(漏洩の根本原因)

`.agents/rules/rust/immutability-policy.md` の AShared パターン規約:

```
AShared パターン(内部可変性の唯一の許容ケース):
inner に ArcShared<SpinSyncMutex<A>> を保持する AShared 構造体を新設
```

**規約レベルで `SpinSyncMutex` を直接指名している**。`RuntimeMutex` は言及すらない。

一方で各 crate の `clippy.toml`:
```toml
{ path = "std::sync::Mutex", reason = "Use impl of SyncMutexLike within production code", replacement = "fraktor_utils_core_rs::sync::SyncMutexLike" },
```
`std::sync::Mutex` を禁止して `SyncMutexLike` impl に誘導している。

**つまり規約と clippy ルールの方向性が食い違っている**:
- 規約: 「`SpinSyncMutex` を直接使え」
- clippy: 「`SyncMutexLike` impl 経由で使え」
- utils の型定義: 「`RuntimeMutex` を使え」

→ 44 ファイルが `SpinSyncMutex` 直接参照に向かったのは規約に従った自然な帰結。設計者ごとに解釈が割れている状態。

### 問題 6: RwLock 側も同様の構造

`SyncRwLockLike` / `RuntimeRwLock` / `StdSyncRwLock` / `SpinSyncRwLock` も同じ構造。漏洩は少ない(6 ファイル、全て utils 内部)が、過剰設計は同様。

### 問題 7: DebugMutex 導入のブロッカー

当初の `DebugMutex` (debug-only 再入検出) 要求を実装しようとすると:
- `SpinSyncMutex` に埋め込むべきか、`StdSyncMutex` に埋め込むべきか、`RuntimeMutex` ラッパーにするべきかが決まらない
- 44 ファイルが `SpinSyncMutex` 直接参照、100+ ファイルが `RuntimeMutex` 経由、という状態ではどこに仕込んでも一部が漏れる
- 先に抽象レイヤを整理しない限り、ガードレールとして機能しない

---

## 数値サマリ

| 型 | 直接参照ファイル数 | 実質用途 |
|----|-------------------|----------|
| `RuntimeMutex` | 100+ | 主流の抽象(ただしバックエンドに価値なし) |
| `SpinSyncMutex` | 44 (うち実プロダクションは ~28) | 実質的な唯一の backend |
| `StdSyncMutex` | 6 | **全て utils 内部の定義とテスト**、実プロダクション利用ゼロ |
| `SpinSyncRwLock` | 6 | 全て utils 内部 |
| `SyncQueueShared` 系 | 8 | **完全デッドコード**(内部テスト以外利用者ゼロ) |

---

## 推奨アクションプラン(選択肢 A: 完全一本化)

### PR 1: デッドコード削除(独立、低リスク)
- `modules/utils/src/core/collections/queue/` から `SyncQueueShared` / `SyncMpsc*Shared` / `SyncSpsc*Shared` と関連テストを削除
- `queue.rs` の `pub use` 削除
- 規模: 8 ファイル削除
- **依存関係なし、独立 PR 可**

### PR 2: `RuntimeMutex` / `RuntimeRwLock` → `SpinSyncMutex` / `SpinSyncRwLock` 機械置換
- 100+ ファイルを機械置換
- テストが通ることを確認(spin::Mutex は同等のセマンティクス)
- `feature = "std"` の依存を削除

### PR 3: 不要型・trait の一括削除
- 削除対象:
  - `RuntimeMutex`, `RuntimeRwLock`, `NoStdMutex` 型エイリアス
  - `RuntimeMutexBackend`, `RuntimeRwLockBackend` (内部)
  - `StdSyncMutex`, `StdSyncMutexGuard`, `StdSyncRwLock`, `StdSyncRwLockReadGuard`, `StdSyncRwLockWriteGuard`
  - `SyncMutexLike`, `SyncRwLockLike` trait
  - `modules/utils/src/std/sync_mutex*`, `modules/utils/src/std/sync_rwlock*` 配下のファイル群
- `clippy.toml` の `disallowed-types` から `std::sync::Mutex` の replacement を更新
  - 新しい replacement: `fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex`
- `utils/Cargo.toml` から `std` feature を削除(使われなくなる)
- `.agents/rules/rust/immutability-policy.md` の整合性確認

### PR 4: `DebugMutex` 相当機能を `SpinSyncMutex` に埋め込む
- `debug_assertions` かつ std が使える環境で再入検出
- `AtomicU64` で thread ID のハッシュを保存(ロックフリー)
- `try_lock` ループで「取得できず、かつ holder == self」なら panic
- Drop で holder クリア
- release ビルドではフィールド自体が消える (ZST 同等)
- 影響範囲: `spin_sync_mutex.rs` + `SpinSyncMutexGuard` 新設(現在は `spin::MutexGuard` を直接返している) = API 破壊的変更

---

## 根本原因の分析

`RuntimeMutex` 抽象は投機的に作られたが、以下の連鎖で崩れた:

1. **想定された唯一の generic consumer(`SyncQueueShared`)が結局使われなかった** → ジェネリクスの価値が消失
2. **規約(`immutability-policy.md`)が `RuntimeMutex` ではなく `SpinSyncMutex` を明示指名した** → 開発者は規約に従い直接参照
3. **`std::sync::Mutex` 固有の挙動(parking, poisoning)が実需として発生しなかった** → バックエンド分岐が無意味化
4. **各モジュールは最短経路(直接参照)を選んだ** → 44 ファイル漏洩

これは「**実需の検証なしに抽象を立てる → 実需が発生しない → 抽象が幽霊化 → 各所で迂回される → 規約が迂回側に寄る**」という典型的な投機的抽象化の失敗パターン。

---

## 別 AI に判断してほしい論点

1. **選択肢 A(完全一本化)で進めて問題ないか?**
  - 代替案 B: `SyncMutexLike` trait だけ残す(将来差し替えの余地)
  - 代替案 C: `RuntimeMutex` 型エイリアスだけ削除、trait と `StdSyncMutex` は残す
2. **`SyncQueueShared` 系のデッドコード削除は本当に安全か?**
  - workspace 外の想定ユーザーがいないか(crates.io 公開されている fraktor-utils-rs は外部利用想定ありそう)
  - 現状 workspace 内で使われていない以上、設計妥当性を検証できない状態なのは確か
3. **`spin::Mutex` 一本化のリスクはないか?**
  - short-held lock 前提のアクターフレームワークで OS parking が本当に不要か
  - ベンチマークなしで決めて良いか
4. **PR 4(`DebugMutex` 埋め込み)の設計確認**
  - `AtomicU64` + thread ID ハッシュ方式は健全か
  - Guard 型を新設する(現状 `spin::MutexGuard` を直接返している)のは受容可能な API 破壊か
  - no_std 環境では検出できない(std::thread が使えないため)のは妥協として妥当か
5. **段階 PR の順序**
  - PR 1 → PR 2 → PR 3 → PR 4 が最小リスクか
  - PR 2 と PR 3 は 1 つにまとめるべきか

---

## 別 AI が調査に使える起点

- `.agents/rules/rust/immutability-policy.md` (AShared 規約)
- `modules/utils/src/lib.rs` (型エイリアス定義)
- `modules/utils/src/core/sync/runtime_lock_alias.rs`
- `modules/utils/src/core/sync/sync_mutex_like.rs`
- `modules/utils/src/core/collections/queue.rs` (デッドコード起点)
- `modules/utils/clippy.toml` / `modules/actor-core/clippy.toml` / `modules/cluster-core/clippy.toml`
- `modules/stream-core/src/core/**/*.rs` の `SpinSyncMutex` 使用箇所
- `modules/actor-core/src/core/kernel/dispatch/mailbox/*.rs` (`RuntimeMutex` を使っているがキュー抽象は独自)

---

以上がサマリです。別 AI にはこのまま貼り付けても読める内容にしてあります。足りない観点や追加で調べておくべき点があれば指示してください。
