# 設計詳細: 動的SupervisorStrategy取得機構

**Change ID**: `add-dynamic-supervisor-strategy`

## アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────┐
│ Props                                                       │
│  ├─ ActorFactory                                            │
│  ├─ MailboxConfig                                           │
│  ├─ DispatcherConfig                                        │
│  └─ Middleware                                              │
│  （SupervisorStrategy指定なし - Pekko互換）                │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ ActorCell生成時
┌─────────────────────────────────────────────────────────────┐
│ ActorCell<TB>                                               │
│  ├─ actor: Mutex<Actor>                                     │
│  ├─ children: Vec<Pid>                                      │
│  └─ （supervisorフィールドなし）                            │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ 子アクター失敗時
┌─────────────────────────────────────────────────────────────┐
│ handle_failure(child, error)                                │
│  1. actor.lock()でActor実装を取得                           │
│  2. actor.supervisor_strategy(ctx)を呼び出し               │
│  3. 返されたSupervisorStrategyを使用                        │
│  4. strategy.handle_failure(stats, error, now)で判定        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ SupervisorDirective                                         │
│  ├─ Restart  → 子アクターを再起動                          │
│  ├─ Stop     → 子アクターを停止                            │
│  └─ Escalate → 親アクターにエスカレート                    │
└─────────────────────────────────────────────────────────────┘
```

## 実装詳細

### 1. Actorトレイトの拡張

**ファイル**: `modules/actor-core/src/actor_prim/actor.rs`

```rust
pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send {
    // 既存のメソッド...
    fn pre_start(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
        Ok(())
    }

    fn receive(
        &mut self,
        ctx: &mut ActorContextGeneric<'_, TB>,
        message: AnyMessageView<'_, TB>,
    ) -> Result<(), ActorError>;

    fn post_stop(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
        Ok(())
    }

    fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>, _terminated: Pid) -> Result<(), ActorError> {
        Ok(())
    }

    /// 子アクターの監督戦略を提供する。
    ///
    /// Actor実装の内部状態に基づいて動的に監督方針を決定できる。
    /// デフォルト実装は`SupervisorStrategy::default()`を返し、Props側では監督戦略を保持しない。
    ///
    /// # 実装例
    ///
    /// ```rust
    /// struct ResilientWorker {
    ///     error_count: u32,
    ///     strict_mode: bool,
    /// }
    ///
    /// impl Actor for ResilientWorker {
    ///     fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
    ///         if self.strict_mode {
    ///             // 厳格モード: 即座に停止
    ///             SupervisorStrategy::new(
    ///                 SupervisorStrategyKind::OneForOne,
    ///                 0,
    ///                 Duration::from_secs(1),
    ///                 |_| SupervisorDirective::Stop
    ///             )
    ///         } else if self.error_count > 10 {
    ///             // エラー多発: 親にエスカレート
    ///             SupervisorStrategy::new(
    ///                 SupervisorStrategyKind::OneForOne,
    ///                 1,
    ///                 Duration::from_secs(5),
    ///                 |_| SupervisorDirective::Escalate
    ///             )
    ///         } else {
    ///             // 通常モード: デフォルト戦略
    ///             SupervisorStrategy::default()
    ///         }
    ///     }
    ///
    ///     fn receive(&mut self, ctx: &mut ActorContext, message: AnyMessageView) -> Result<(), ActorError> {
    ///         // メッセージ処理でerror_countを更新
    ///         // ...
    ///         Ok(())
    ///     }
    /// }
    /// ```
    ///
    /// # 注意事項
    ///
    /// - このメソッドは子アクターの失敗時に呼び出される
    /// - 頻繁に呼ばれるわけではないが、軽量な実装を推奨
    /// - 戦略の決定ロジックは状態更新を含めることができる
    /// - 既存のActor traitメソッド（receive, pre_startなど）と一貫したシグネチャ
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
        SupervisorStrategy::default()
    }
}
```

### 2. ActorCellの変更

**ファイル**: `modules/actor-core/src/actor_prim/actor_cell.rs`

#### 2.1 構造体フィールド削除

```rust
pub struct ActorCell<TB: RuntimeToolbox> {
    pid:         Pid,
    parent:      Option<Pid>,
    actor:       ToolboxMutex<Box<dyn Actor<TB>>, TB>,
    sender:      ArcShared<DispatcherSenderGeneric<TB>>,
    children:    ToolboxMutex<Vec<Pid>, TB>,

    // supervisor: SupervisorStrategy,  // ← 削除

    child_stats: ToolboxMutex<Vec<(Pid, RestartStatistics)>, TB>,
    watchers:    ToolboxMutex<Vec<Pid>, TB>,
    // ...その他のフィールド
}
```

**変更点**:
- `supervisor`フィールドを削除（Props由来の固定戦略が不要になった）
- メモリ使用量が約48バイト削減

#### 2.2 コンストラクタの変更

```rust
impl<TB: RuntimeToolbox> ActorCell<TB> {
    pub fn new(
        pid: Pid,
        parent: Option<Pid>,
        props: &PropsGeneric<TB>,
        sender: ArcShared<DispatcherSenderGeneric<TB>>,
    ) -> Result<ArcShared<Self>, ActorError> {
        let factory = props.factory().clone();
        let actor = <TB::MutexFamily as SyncMutexFamily>::create(factory.create());
        let children = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());

        // let supervisor = *props.supervisor().strategy();  // ← 削除

        let child_stats = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());
        let watchers = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());

        let cell = ArcShared::new(Self {
            pid,
            parent,
            actor,
            sender,
            children,
            // supervisor,  // ← 削除
            child_stats,
            watchers,
            // ...
        });

        Ok(cell)
    }
}
```

#### 2.3 handle_failureの変更

```rust
impl<TB: RuntimeToolbox> ActorCell<TB> {
    fn handle_failure(&self, child: Pid, error: ActorError) -> FailureOutcome {
        let now = TB::Clock::now();

        // 監督戦略を動的に取得
        let strategy = {
            let system = ActorSystemGeneric::from_state(self.system.clone());
            let mut ctx = ActorContextGeneric::new(&system, self.pid);
            let mut actor = self.actor.lock();
            // Actor実装から戦略を取得
            actor.supervisor_strategy(&mut ctx)
        };

        // 戦略に基づいてディレクティブを決定
        let directive = {
            let mut stats = self.child_stats.lock();
            let entry = find_or_insert_stats(&mut stats, child);
            strategy.handle_failure(entry, &error, now)
        };

        // 影響を受ける子アクターを決定
        let affected = match strategy.kind() {
            SupervisorStrategyKind::OneForOne => vec![child],
            SupervisorStrategyKind::AllForOne => self.children.lock().clone(),
        };

        // ディレクティブを適用
        match directive {
            SupervisorDirective::Restart => {
                for pid in affected {
                    self.restart_child(pid);
                }
                FailureOutcome::Restart
            }
            SupervisorDirective::Stop => {
                for pid in affected {
                    self.stop_child(pid);
                }
                FailureOutcome::Stop
            }
            SupervisorDirective::Escalate => {
                // 親にエスカレート
                FailureOutcome::Escalate
            }
        }
    }

    fn create_context(&self) -> ActorContextGeneric<'_, TB> {
        // ActorContextを構築するヘルパー
        // 既存のコードから流用
        // ...
    }
}
```

### 3. stdモジュールの対応

**ファイル**: `modules/actor-std/src/actor_prim/actor.rs`

coreモジュールと同じ変更を適用:

```rust
pub trait Actor: Send {
    // 既存のメソッド...

    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategy {
        SupervisorStrategy::default()
    }
}

### 4. SupervisorStrategyのデフォルト実装

`modules/actor-core/src/supervision/base.rs`に`impl Default for SupervisorStrategy`を追加し、従来`SupervisorOptions::default()`が提供していた挙動を移植する。

```rust
impl Default for SupervisorStrategy {
    fn default() -> Self {
        const fn decider(err: &ActorError) -> SupervisorDirective {
            match err {
                ActorError::Recoverable(_) => SupervisorDirective::Restart,
                ActorError::Fatal(_) => SupervisorDirective::Stop,
            }
        }

        SupervisorStrategy::new(
            SupervisorStrategyKind::OneForOne,
            10,
            Duration::from_secs(1),
            decider,
        )
    }
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self::new(SupervisorStrategy::default())
    }
}
```

この変更により、`supervisor_strategy`をオーバーライドしない既存Actorは以前と同じ挙動（Recoverable→Restart / Fatal→Stop）を維持しつつ、Propsから戦略を参照する必要がなくなる。
```

## データフロー

### 基本フロー（デフォルト戦略を使用）

```
1. Props生成
   └─ ActorFactory, MailboxConfig, DispatcherConfig等のみ

2. ActorCell生成
   └─ Props由来のsupervisorフィールドなし

3. 子アクター失敗
   └─ actor.supervisor_strategy(ctx) → SupervisorStrategy::default()
   └─ デフォルト戦略を使用（OneForOne, 10回再起動, 1秒以内）
```

### 動的フロー（Actor実装がカスタム戦略を提供）

```
1. Props生成
   └─ ActorFactory, MailboxConfig, DispatcherConfig等のみ

2. ActorCell生成
   └─ Props由来のsupervisorフィールドなし

3. 子アクター失敗
   └─ actor.supervisor_strategy(ctx) → カスタムSupervisorStrategy
   └─ Actor状態に基づく動的戦略を使用
```

## パフォーマンス分析

### 追加コスト

| 操作 | コスト | 頻度 | 影響 |
|------|--------|------|------|
| `ActorCell`構造体サイズ削減 | -48バイト程度 | アクター生成時 | メモリ効率向上 |
| `actor.lock()` | Mutexロック | 子失敗時のみ | 軽微 |
| `supervisor_strategy()` 呼び出し | メソッド呼び出し | 子失敗時のみ | 最小限 |

### ホットパス影響

- **メッセージ処理**: 影響なし（`receive`はそのまま）
- **通常動作**: 影響なし（失敗時のみ追加コスト）
- **失敗処理**: 軽微な追加コスト（Mutexロック1回 + メソッド呼び出し1回）

#### 計測手順

- `cargo bench -p actor-core supervisor_failures`（新規ベンチ）で旧実装と新実装の失敗処理時間を比較し、差分をproposal/designに追記する
- `std::mem::size_of::<ActorCellGeneric<StdToolbox>>()` と `::<SupervisorStrategy>()` を実測し、約48バイトの削減が成立しているか検証する

## エッジケースの処理

### ケース1: 再帰的失敗

子アクターの失敗処理中にさらに失敗が発生する場合:

```rust
// handle_failure内でエラーが発生しないよう設計
fn handle_failure(&self, child: Pid, error: ActorError) -> FailureOutcome {
    // panic!を避け、Resultではなく直接Outcomeを返す
    // ロック取得も最小限にする
}
```

### ケース2: Escalate連鎖

親アクターも`Escalate`を返す場合、最終的にルートに到達:

```rust
// ActorSystemのルートガーディアンで停止
// 既存のエスカレート処理をそのまま使用
```

### ケース3: Actor実装が`panic!`する場合（非推奨）

**重要**: `supervisor_strategy`メソッド実装はpanic-freeであるべきです。

**no_std環境での動作**:
- `panic = abort`: アプリケーション全体が即座に終了
- ライブラリは関与しない（panic回復メカニズムなし）

**std環境（panic = unwind）での動作**:
- Mutexがpoisonedになり、以降のロック取得が失敗
- システムの予測可能な動作が保証されない
- ライブラリは関与せず、アプリケーション側の責任

**推奨実装**:
```rust
// ✅ panic-free実装
fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
    if self.error_count > 10 {
        SupervisorStrategy::stopping()
    } else {
        SupervisorStrategy::default()
    }
}
```

## テスト戦略

### ユニットテスト

1. **動的戦略変更**
   - Actor内部状態を変更して返される`SupervisorStrategy`が切り替わることを確認
   - OneForOne/AllForOne/Escalateの全パターンを網羅

2. **デフォルト実装**
   - `supervisor_strategy`をオーバーライドしないActorが`SupervisorStrategy::default()`を返すことを確認

3. **OneForOne vs AllForOne**
   - 動的に戦略種別が変わることを確認

### 統合テスト

1. **エスカレート動作**
   - Actor実装が`Escalate`を返す戦略を提供した場合の動作

2. **状態に基づく判断**
   - エラーカウント増加に伴う戦略変更

3. **既存テストの継続性**
   - `modules/actor-core/tests/supervisor.rs`が引き続き動作

## 互換性マトリクス

| コンポーネント | 変更 | 互換性 |
|----------------|------|--------|
| `Actor` trait | `supervisor_strategy`追加（デフォルト実装が`SupervisorStrategy::default()`を返す） | ✅ 後方互換 |
| `ActorCell` | `supervisor`フィールド削除 + `handle_failure`を動的取得に変更 | ⚠️ 内部実装のみ（公開API変更なし） |
| `Props` | `with_supervisor`/`supervisor` API削除 | ❌ 破壊的（移行ガイド必須） |
| `SupervisorStrategy` | `Copy`→`Clone`へ変更 | ❌ 破壊的（`.clone()`追記が必要） |
| 既存のActor実装 | デフォルト実装で動作 | ✅ 互換 |

## ロールバック戦略

本変更は以下の点でロールバックが容易:

1. **Actorトレイトのデフォルト実装**
   - メソッド削除で元に戻る

2. **ActorCellのフィールド差分のみ**
   - `supervisor`フィールドを復活させ、`handle_failure`呼び出しを元に戻せばロールバック可能

3. **Props APIの巻き戻し**
   - `Props::with_supervisor`を再導入し、Actor traitメソッドを削除すれば従来挙動に戻せる（ただし一括ロールバックが必要）

## 将来の拡張

### Typed Actorとの統合

```rust
// Behaviors.supervise DSLとの併用
let behavior = Behaviors::setup(|ctx| {
    // Untyped層のActor実装も内部で使用可能
    // 両方の戦略が定義された場合、Behaviorレベルが優先
});
```

### クロージャベースのdecider

`Copy`制約をすでに削除したため、将来的には以下のようにクロージャを格納できる:

```rust
pub struct SupervisorStrategy {
    kind: SupervisorStrategyKind,
    max_restarts: u32,
    within: Duration,
    decider: Box<dyn Fn(&ActorError) -> SupervisorDirective + Send>,  // Boxで格納
}
```

## まとめ

本設計は以下の特性を持つ:

✅ **シンプル**: 最小限の変更で実現
✅ **型安全**: Actorトレイトを通じた静的型付け
✅ **柔軟**: Actor状態に基づく動的判断が可能（`&mut self`により状態更新可能）
⚠️ **互換**: `Props::with_supervisor`と`SupervisorStrategy: Copy`に依存するコードは移行が必要
✅ **Pekko互換**: Classic ActorのsupervisorStrategyメソッドに相当
✅ **no_std対応**: panic-free実装を要求、ライブラリは関与しない
