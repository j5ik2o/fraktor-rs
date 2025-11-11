# 動的SupervisorStrategy移行ガイド

## 概要

このガイドでは、`Props`ベースの固定監督戦略から`Actor`トレイトベースの動的監督戦略への移行方法を説明します。

## 破壊的変更

### 削除されたAPI

- `Props::with_supervisor(SupervisorOptions) -> Props`
- `Props::supervisor() -> &SupervisorOptions`
- `SupervisorStrategy` と `SupervisorOptions` の `Copy` トレイト

### 追加されたAPI

- `Actor::supervisor_strategy(&mut self, &mut ActorContext) -> SupervisorStrategy`
- `SupervisorStrategy::default()` - デフォルト戦略を返す

## 移行手順

### Before (旧実装)

```rust
use fraktor_actor_std_rs::{
    actor_prim::Actor,
    props::Props,
};
use fraktor_actor_core_rs::{
    props::SupervisorOptions,
    supervision::{SupervisorStrategy, SupervisorStrategyKind, SupervisorDirective},
};

struct MyWorker;

impl Actor for MyWorker {
    fn receive(&mut self, ctx: &mut ActorContext, message: AnyMessageView) -> Result<(), ActorError> {
        // ... message handling
        Ok(())
    }
}

// Props経由で戦略を指定
let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    3,
    Duration::from_secs(1),
    |error| match error {
        ActorError::Recoverable(_) => SupervisorDirective::Restart,
        ActorError::Fatal(_) => SupervisorDirective::Stop,
    },
);

let props = Props::from_fn(MyWorker::new)
    .with_supervisor(SupervisorOptions::new(strategy));  // ❌ 削除されました
```

### After (新実装)

```rust
use fraktor_actor_std_rs::{
    actor_prim::Actor,
    props::Props,
};
use fraktor_actor_core_rs::{
    supervision::{SupervisorStrategy, SupervisorStrategyKind, SupervisorDirective},
};

struct MyWorker;

impl Actor for MyWorker {
    fn receive(&mut self, ctx: &mut ActorContext, message: AnyMessageView) -> Result<(), ActorError> {
        // ... message handling
        Ok(())
    }

    // Actor自身が戦略を提供
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
        SupervisorStrategy::new(
            SupervisorStrategyKind::OneForOne,
            3,
            Duration::from_secs(1),
            |error| match error {
                ActorError::Recoverable(_) => SupervisorDirective::Restart,
                ActorError::Fatal(_) => SupervisorDirective::Stop,
            },
        )
    }
}

// Propsはシンプルに
let props = Props::from_fn(MyWorker::new);  // ✅ 戦略指定は不要
```

## 動的戦略の利点

新しい実装では、アクターの状態に基づいて監督戦略を動的に変更できます:

```rust
struct AdaptiveWorker {
    consecutive_errors: u32,
}

impl Actor for AdaptiveWorker {
    fn receive(&mut self, ctx: &mut ActorContext, message: AnyMessageView) -> Result<(), ActorError> {
        match process_message(message) {
            Ok(_) => {
                self.consecutive_errors = 0;
                Ok(())
            }
            Err(e) => {
                self.consecutive_errors += 1;
                Err(e)
            }
        }
    }

    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
        if self.consecutive_errors > 10 {
            // エラーが多すぎる場合は即停止
            SupervisorStrategy::stopping()
        } else {
            // 通常時は再起動を許可
            SupervisorStrategy::default()
        }
    }
}
```

## デフォルト戦略

`supervisor_strategy()`メソッドをオーバーライドしない場合、デフォルト戦略が使用されます:

- Strategy kind: **OneForOne** (失敗した子アクターのみ再起動)
- Maximum restarts: **10回**
- Time window: **1秒**
- Decider:
  - `Recoverable` → `Restart`
  - `Fatal` → `Stop`

この戦略は、以前の`SupervisorOptions::default()`と同じです。

## 注意事項

### `Copy`トレイトの削除

`SupervisorStrategy`と`SupervisorOptions`は`Copy`ではなくなりました。必要に応じて`.clone()`を使用してください:

```rust
// Before
let strategy1 = SupervisorStrategy::default();
let strategy2 = strategy1;  // Copyが自動的に使われる
use_both(strategy1, strategy2);  // OK

// After
let strategy1 = SupervisorStrategy::default();
let strategy2 = strategy1.clone();  // 明示的なcloneが必要
use_both(strategy1, strategy2);  // OK
```

### panic-free要件

`supervisor_strategy()`メソッドは**panic-freeである必要があります**。このメソッドは障害処理中に呼ばれるため、panicが発生するとシステムが不安定になります（特にno_std環境）。

```rust
// ❌ 悪い例
fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
    self.config.as_ref().unwrap().strategy()  // panic可能性あり
}

// ✅ 良い例
fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
    self.config
        .as_ref()
        .map(|c| c.strategy())
        .unwrap_or_else(SupervisorStrategy::default)
}
```

## 参考

- Pekko Classic: [`Actor#supervisorStrategy`](https://pekko.apache.org/api/pekko/1.1/org/apache/pekko/actor/Actor.html#supervisorStrategy:org.apache.pekko.actor.SupervisorStrategy)
- OpenSpec提案: `openspec/changes/add-dynamic-supervisor-strategy/`
