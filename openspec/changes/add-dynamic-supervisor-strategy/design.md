# 設計詳細: 動的SupervisorStrategy取得機構

**Change ID**: `add-dynamic-supervisor-strategy`

## アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────┐
│ Props                                                       │
│  ├─ SupervisorOptions                                       │
│  │   └─ SupervisorStrategy (デフォルト戦略)                │
│  └─ ActorFactory                                            │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ ActorCell生成時
┌─────────────────────────────────────────────────────────────┐
│ ActorCell<TB>                                               │
│  ├─ actor: Mutex<Actor>                                     │
│  ├─ default_supervisor: SupervisorStrategy (Propsから)      │
│  └─ children: Vec<Pid>                                      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ 子アクター失敗時
┌─────────────────────────────────────────────────────────────┐
│ handle_failure(child, error)                                │
│  1. actor.lock()でActor実装を取得                           │
│  2. actor.supervisor_strategy(ctx)を呼び出し               │
│  3. Some(strategy) → その戦略を使用                         │
│     None           → default_supervisorを使用               │
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
    /// `None`を返した場合、`Props`で指定されたデフォルト戦略が使用される。
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
    ///     fn supervisor_strategy(&self, _ctx: &ActorContext) -> Option<SupervisorStrategy> {
    ///         if self.strict_mode {
    ///             // 厳格モード: 即座に停止
    ///             Some(SupervisorStrategy::new(
    ///                 SupervisorStrategyKind::OneForOne,
    ///                 0,
    ///                 Duration::from_secs(1),
    ///                 |_| SupervisorDirective::Stop
    ///             ))
    ///         } else if self.error_count > 10 {
    ///             // エラー多発: 親にエスカレート
    ///             Some(SupervisorStrategy::new(
    ///                 SupervisorStrategyKind::OneForOne,
    ///                 1,
    ///                 Duration::from_secs(5),
    ///                 |_| SupervisorDirective::Escalate
    ///             ))
    ///         } else {
    ///             // 通常モード: Propsのデフォルト戦略を使用
    ///             None
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
    /// - 戦略の決定ロジックに副作用を持たせない（純粋関数として実装）
    fn supervisor_strategy(&self, _ctx: &ActorContextGeneric<'_, TB>) -> Option<SupervisorStrategy> {
        None
    }
}
```

### 2. ActorCellの変更

**ファイル**: `modules/actor-core/src/actor_prim/actor_cell.rs`

#### 2.1 構造体フィールド追加

```rust
pub struct ActorCell<TB: RuntimeToolbox> {
    pid:         Pid,
    parent:      Option<Pid>,
    actor:       ToolboxMutex<Box<dyn Actor<TB>>, TB>,
    sender:      ArcShared<DispatcherSenderGeneric<TB>>,
    children:    ToolboxMutex<Vec<Pid>, TB>,

    // 追加: Propsから取得したデフォルト戦略
    default_supervisor: SupervisorStrategy,

    child_stats: ToolboxMutex<Vec<(Pid, RestartStatistics)>, TB>,
    watchers:    ToolboxMutex<Vec<Pid>, TB>,
    // ...その他のフィールド
}
```

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

        // Propsから戦略を取得（コピー）
        let default_supervisor = *props.supervisor().strategy();

        let child_stats = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());
        let watchers = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());

        let cell = ArcShared::new(Self {
            pid,
            parent,
            actor,
            sender,
            children,
            default_supervisor,  // 追加
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
            let actor = self.actor.lock();
            // Actor実装から戦略を取得、なければデフォルト
            actor.supervisor_strategy(&self.create_context())
                .unwrap_or(self.default_supervisor)
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

    fn supervisor_strategy(&self, _ctx: &ActorContext<'_>) -> Option<SupervisorStrategy> {
        None
    }
}
```

## データフロー

### 通常フロー（Actor実装が戦略を提供しない場合）

```
1. Props生成
   └─ SupervisorOptions::default() → デフォルト戦略

2. ActorCell生成
   └─ default_supervisor = props.supervisor().strategy()

3. 子アクター失敗
   └─ actor.supervisor_strategy(ctx) → None
   └─ default_supervisor を使用
```

### 動的フロー（Actor実装が戦略を提供する場合）

```
1. Props生成
   └─ SupervisorOptions::default() → デフォルト戦略

2. ActorCell生成
   └─ default_supervisor = props.supervisor().strategy()

3. 子アクター失敗
   └─ actor.supervisor_strategy(ctx) → Some(custom_strategy)
   └─ custom_strategy を使用
```

## パフォーマンス分析

### 追加コスト

| 操作 | コスト | 頻度 | 影響 |
|------|--------|------|------|
| `ActorCell`構造体サイズ増加 | +48バイト程度 | アクター生成時 | 最小限 |
| `actor.lock()` | Mutexロック | 子失敗時のみ | 軽微 |
| `supervisor_strategy()` 呼び出し | メソッド呼び出し | 子失敗時のみ | 最小限 |

### ホットパス影響

- **メッセージ処理**: 影響なし（`receive`はそのまま）
- **通常動作**: 影響なし（失敗時のみ追加コスト）
- **失敗処理**: 軽微な追加コスト（Mutexロック1回 + メソッド呼び出し1回）

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

### ケース3: Actor実装が`panic!`する場合

`supervisor_strategy`メソッド内でパニックした場合:

```rust
// Mutexがpoisonedになるが、既存のエラー処理で対応
// 最悪の場合、デフォルト戦略にフォールバック
let strategy = {
    let actor = self.actor.lock();
    match actor.supervisor_strategy(&ctx) {
        Some(s) => s,
        None => self.default_supervisor,
    }
};
```

## テスト戦略

### ユニットテスト

1. **動的戦略変更**
   - Actor内部状態を変更して戦略が切り替わることを確認
   - `Some(strategy)`と`None`の両パターン

2. **デフォルトフォールバック**
   - `None`を返した場合にPropsの戦略が使われることを確認

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
| `Actor` trait | デフォルト実装付きメソッド追加 | ✅ 後方互換 |
| `ActorCell` | フィールド追加 | ⚠️ 内部実装（公開APIではない） |
| `Props` | 変更なし | ✅ 互換 |
| `SupervisorStrategy` | Copy維持 | ✅ 互換 |
| 既存のActor実装 | 変更不要 | ✅ 互換 |

## ロールバック戦略

本変更は以下の点でロールバックが容易:

1. **Actorトレイトのデフォルト実装**
   - メソッド削除で元に戻る

2. **ActorCellのフィールド追加のみ**
   - フィールド削除と元のロジックに戻すだけ

3. **破壊的変更なし**
   - 既存コードは変更なしで動作

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

`Copy`制約を削除する場合:

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
✅ **柔軟**: Actor状態に基づく動的判断が可能
✅ **互換**: 既存コードへの影響が最小限
✅ **Pekko互換**: Classic ActorのsupervisorStrategyメソッドに相当
