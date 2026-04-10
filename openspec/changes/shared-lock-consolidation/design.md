## Context

fraktor-rs の同期プリミティブ層（`modules/utils-core/src/core/sync/`）には以下の型が存在する：

| 型 | 責務 |
|---|---|
| `LockDriver<T>` / `RwLockDriver<T>` | ドライバ trait（GAT 持ち、object-unsafe） |
| `SpinSyncMutex<T>` / `SpinSyncRwLock<T>` | デフォルトのスピンベース実装 |
| `RuntimeMutex<T, D>` / `RuntimeRwLock<T, D>` | ドライバの薄いラッパー（`D` が型パラメータとして露出） |
| `SharedLock<T>` | `D` を型消去 + `ArcShared` 内蔵の closure-based 共有ラッパー |

問題：
1. `RuntimeMutex`/`RuntimeRwLock` は `LockDriver`/`RwLockDriver` への単純委譲で独自の価値がない
2. `SharedLock` が内部で `RuntimeMutex` を経由しており地層化している
3. RwLock 系に `SharedLock` 相当の型消去ラッパーがなく、`ArcShared<RuntimeRwLock<T>>` を直接使っている

`ActorLockProvider` はドライバを差し替える仕組みを提供しており、`SharedLock` を生成して返す。`RuntimeMutex` は `ActorLockProvider` を経由しない no_std デフォルト使用で残存しているが、これも `SharedLock::new_with_driver::<SpinSyncMutex<T>>` で代替可能。

## Goals / Non-Goals

**Goals:**
- `SharedLock<T>` の内部から `RuntimeMutex` 経由を除去し、`LockDriver` を直接保持する
- `SharedRwLock<T>` を新設し、`SharedLock` と対称な API を提供する
- `RuntimeMutex<T, D>` と `RuntimeRwLock<T, D>` を全使用箇所から段階的に除去し、最終的に廃止する
- `NoStdMutex<T>` 型エイリアスを廃止する

**Non-Goals:**
- `LockDriver`/`RwLockDriver` trait 自体の変更（これらは今後もドライバ実装の契約として残る）
- `LockDriverFactory`/`RwLockDriverFactory` の変更
- `SpinSyncMutex`/`SpinSyncRwLock` の変更
- `ActorLockProvider` trait の変更（返却型が既に `SharedLock`。RwLock のドライバ差し替え需要は現時点でないため YAGNI）
- guard 返却 API の新設（closure-based API に統一する）
- `SharedAccess` trait 自体の変更（既存の `with_read`/`with_write` シグネチャは維持）

## Decisions

### D1: `SharedLock` 内部の地層解消

`RuntimeMutexSharedLockBackend<T, D>` が `RuntimeMutex<T, D>` を保持する構造を、`D: LockDriver<T>` を直接保持する構造に変更する。

**Before:**
```
SharedLock<T>
  → ArcShared<dyn SharedLockBackend<T>>
    → RuntimeMutexSharedLockBackend<T, D>
      → RuntimeMutex<T, D>
        → D: LockDriver<T>
```

**After:**
```
SharedLock<T>
  → ArcShared<dyn SharedLockBackend<T>>
    → LockDriverSharedLockBackend<T, D>
      → D: LockDriver<T>
```

**理由:** `RuntimeMutex` は `D::lock()` に委譲するだけ。中間層を除去しても動作は同一。

### D2: `SharedRwLock<T>` の設計

`SharedLock<T>` と対称に設計する。

```rust
trait SharedRwLockBackend<T>: Send + Sync {
  fn with_read(&self, f: &mut dyn FnMut(&T));
  fn with_write(&self, f: &mut dyn FnMut(&mut T));
}

pub struct SharedRwLock<T> {
  inner: ArcShared<dyn SharedRwLockBackend<T>>,
}
```

API:
- `SharedRwLock::new_with_driver::<D>(value: T) -> Self`
- `SharedRwLock::with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R`
- `SharedRwLock::with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R`
- `impl Clone for SharedRwLock<T>`

**理由:** `RwLockDriver` も GAT を持つため object-unsafe。`SharedLock` と同じ closure-based 型消去パターンが必要。

### D3: `RuntimeMutex` / `RuntimeRwLock` 使用箇所の移行戦略

使用パターンは4種類ある：

| パターン | 件数 | 移行先 |
|---|---|---|
| A: `ArcShared<RuntimeMutex<T>>` として `*Shared` 型内部で共有 | 多数（`ActorShared`, `CellsShared`, `CircuitBreakerShared` 等） | `SharedLock<T>`（二重 Arc、後述の判定基準で許容/剥がし判断） |
| B: `RuntimeMutex<T>` を非共有フィールドとして直接保持 | 約25箇所（`ActorCell`, `CoordinatedShutdown`, MessageQueue 各種, `TickDriverConfig`, `AdaptMessage`, `BatchingProducerState` 等） | `SharedLock<T>`（`ArcShared` 内蔵だが共有不要なため不要な Arc が付く。ただし guard 排除の一貫性を優先） |
| C: `SharedLock<T>` 経由（`ActorLockProvider` 系） | 少数（`MessageDispatcherShared`, `ExecutorShared`, `ActorRefSenderShared`） | 変更不要（既に `SharedLock` 使用中） |
| D: `ArcShared<RuntimeRwLock<T>>` として `*Shared` 型内部で共有 | 約14箇所（`EventStreamShared`, `SchedulerShared` 等） | `SharedRwLock<T>` |
| E: `RuntimeRwLock<T>` を非共有フィールドとして直接保持 | 少数（`SerializationRegistry` の `serializers`, `bindings`, `cache` 等5フィールド） | `SharedRwLock<T>`（パターン B と同じ理由で許容） |

**パターン B の詳細:**
非共有フィールドに `RuntimeMutex<T>` を直接持つケースは以下に分類される：

- **MessageQueue 実装**: `UnboundedDequeMessageQueue`, `BoundedPriorityMessageQueue` 等（5種）。`MessageQueue` trait のメソッドが `&self` であるため内部可変性が必要。`SharedLock` に移行する。
- **ActorCell の内部状態**: `receive_timeout`, `state`。セルは `ArcShared<ActorCell>` で共有されるため、内部の `SharedLock` で二重 Arc になる。判定基準に従い許容。
- **CoordinatedShutdown**: `tasks`, `reason`, `delay_provider`。自身が `ArcShared` で共有される。同上。
- **TickDriverConfig**: `driver`, `executor_pump`。構築時に一度だけ使われ、共有されない。`SharedLock` にすると不要な Arc 割り当てが生じるが、構築パスのため性能影響は negligible。
- **AdaptMessage / AdapterEnvelope**: `payload`。`Option` の take 用。`SharedLock` に移行。
- **cluster-core**: `ClusterExtensionInstaller` の `subscription`, `terminated`, `BatchingProducerState` の `state`。
- **persistence-core**: `JournalActorAdapter`, `SnapshotActorAdapter` の `inner`。

**二重 Arc の判定基準:**

| 条件 | 判定 | 理由 |
|---|---|---|
| 当該型自体が `ArcShared` で包まれて共有されている | 許容 | 将来的に外側 `ArcShared` を `SharedLock` 内蔵の Arc に統合する余地がある |
| 当該型が所有権一意（共有されない） | 許容 | 不要な Arc が付くが、構築パスのみの一回コストで性能影響は negligible |
| hot path で毎回構築される | **剥がす** | `SharedLock` を使わず `SpinSyncMutex<T>` を直接フィールドに持つ。`LockDriver` の guard API を直接使用 |

現時点で hot path 構築に該当する箇所はない。すべて初期化時の一回構築のため、全箇所で `SharedLock` 統一を許容する。

### D4: `SharedAccess` trait との統合

既存の `SharedAccess<B>` trait は `with_read` / `with_write` を提供しており、`SharedLock`/`SharedRwLock` の closure API と同じシグネチャ。現在24箇所の `*Shared` 型が手動で `impl SharedAccess` を書いている。

方針:
- `SharedLock<T>` に `impl SharedAccess<T> for SharedLock<T>` を追加（`with_read` / `with_write` ともに内部で `with_lock` に委譲）
- `SharedRwLock<T>` に `impl SharedAccess<T> for SharedRwLock<T>` を追加（`with_read` は共有ロック、`with_write` は排他ロック）
- `*Shared` 型が内部に `SharedLock`/`SharedRwLock` を持つ場合、`impl SharedAccess` は委譲コードが削減される
- **`SharedLock` の固有メソッド `with_read` は廃止する**。`SharedLock` の固有 API は `with_lock` のみとし、`with_read` / `with_write` は `SharedAccess` trait 実装経由でのみ提供する。理由：`SharedLock::with_read` は「排他ロックで読み取り」だが、名前が「read lock を取っている」と誤解を招く。`SharedRwLock::with_read`（共有ロック）とセマンティクスが異なるのに同じ固有メソッド名を持つことを避ける

API の使い分け:
- `SharedLock` を直接使うとき → `with_lock`（排他ロック、明示的）
- `SharedAccess` trait 経由で抽象化するとき → `with_read` / `with_write`（ロック戦略は実装依存）

**Mutex → RwLock への切り替え判定基準:**
`ArcShared<RuntimeMutex<T>>` の移行時に `SharedLock`（排他）と `SharedRwLock`（共有読み取り）のどちらにすべきかの判断基準：

| 条件 | 移行先 | 理由 |
|---|---|---|
| 既存コードが `RuntimeRwLock` を使っている | `SharedRwLock` | 設計者が共有読み取りを意図している |
| `with_read` の呼び出しが `with_write` より圧倒的に多く、読み取り競合が予想される | `SharedRwLock` への変更を検討 | ただし本変更のスコープ外。別 change として提案 |
| それ以外の `RuntimeMutex` 使用箇所 | `SharedLock` | 現状のセマンティクス（排他ロック）を維持。Mutex → RwLock への変更は振る舞い変更であり、本変更のスコープに含めない |

重要: **本変更では Mutex ↔ RwLock のセマンティクス変更を行わない**。既存の排他ロック箇所は `SharedLock` に、既存の共有読み取り箇所は `SharedRwLock` に、それぞれ同じセマンティクスで移行する。パフォーマンス最適化としての Mutex → RwLock 切り替えは別 change で検討する。

### D5: `WeakShared` との互換性

`SystemStateWeak` は `WeakShared<RuntimeRwLock<SystemState>>` を保持している。`SharedRwLock` は `ArcShared` を内蔵するため、外部から `WeakShared` を作る手段がそのままではない。

対処方針: `SharedRwLock<T>` に `downgrade()` メソッドを追加し、`WeakSharedRwLock<T>` を返す。

```rust
pub struct WeakSharedRwLock<T> {
  inner: WeakShared<dyn SharedRwLockBackend<T>>,
}

impl<T> WeakSharedRwLock<T> {
  pub fn upgrade(&self) -> Option<SharedRwLock<T>> { ... }
}
```

同様に `SharedLock<T>` にも `downgrade()` → `WeakSharedLock<T>` を追加する（対称性のため）。

これにより `SystemStateWeak` は `WeakSharedRwLock<SystemState>` を保持するようになる。

**ファイル配置:** `WeakSharedLock<T>` は `shared_lock.rs` に、`WeakSharedRwLock<T>` は `shared_rw_lock.rs` に同居させる。いずれも親型の downgrade 専用で単独使用されないため、type-per-file の同居条件（親型のメソッド戻り値としてのみ使用、≤20行）を満たす。

### D7: 段階的移行の順序

1. **Phase 1**: `SharedLock` 内部の地層解消（`RuntimeMutex` 経由を除去）
2. **Phase 2**: `SharedRwLock<T>` を新設
3. **Phase 3**: 非推奨マークの付与（CI は `--force-warn deprecated` のため警告のまま通る。コンパイラが残存箇所を教えてくれるため移行漏れ防止に有効）
4. **Phase 4**: `ArcShared<RuntimeRwLock<T>>` → `SharedRwLock<T>` に移行（約14箇所。deprecated 警告を手がかりに特定）
5. **Phase 5**: `ArcShared<RuntimeMutex<T>>` パターンと `RuntimeMutex<T>` 直接保持パターンを `SharedLock` に移行（多数。guard チェーンの closure 化を含む）
6. **Phase 6**: `RuntimeMutex`, `RuntimeRwLock`, `NoStdMutex` を廃止（deprecated 警告ゼロを確認してから削除）

Phase 1-2 は独立。Phase 3 は 1-2 完了後。Phase 4-5 は並行可能。Phase 6 は 4-5 完了後。

## Risks / Trade-offs

- **[guard チェーンの closure 化]** `ArcShared<RuntimeMutex<T>>` パターンでは `.lock().method()` のガードチェーンが大量に使われている（`.lock()` 呼び出しが全体で約1400箇所以上）。`SharedLock` 移行時にすべて `.with_lock(|v| v.method())` に書き換える必要がある。機械的だがボリュームが大きい → Phase 5 を対象ディレクトリごとに分割し、段階的に移行。
- **[closure API への統一]** guard 返却 API がなくなるため、ネストした lock 区間が必要な箇所では closure のネストが深くなる可能性がある → 該当箇所はプロジェクトポリシー（ロック区間はメソッド内に閉じる）により少数。個別に対処。
- **[二重 Arc]** `SharedLock`/`SharedRwLock` が `ArcShared` を内蔵するため、既に `ArcShared` で包まれた構造体のフィールドとして使うと二重 Arc になる → メモリオーバーヘッドは軽微。一貫性を優先。
- **[大規模リファクタリング]** `ArcShared<RuntimeMutex<T>>` パターンが多数のファイルに分散。移行漏れのリスク → Phase ごとに CI を通して段階的に確認。
- **[dyn dispatch オーバーヘッド]** `SharedLock`/`SharedRwLock` は `dyn SharedLockBackend` を介するため仮想関数呼び出しが増える → hot path では `RuntimeMutex` も guard 経由で同等のコスト。ロック取得自体のコストに比べれば negligible。
