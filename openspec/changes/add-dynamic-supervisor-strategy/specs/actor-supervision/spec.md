# Actor Supervision Capability Specification

**Capability ID**: `actor-supervision`
**Version**: 1.0.0 (ADDED by add-dynamic-supervisor-strategy)

## ADDED Requirements

### Requirement: Actor実装による監督戦略の提供 (REQ-001)

Actor実装は`supervisor_strategy`メソッドを通じて、子アクターの失敗に対する監督戦略を動的に提供できなければならない (MUST)。

**優先度**: HIGH

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor.rs`
- `modules/actor-std/src/actor_prim/actor.rs`

#### Scenario: Actor実装がカスタム戦略を提供する

**Given**: Actorが内部状態としてエラーカウントを持つ

```rust
struct ResilientWorker {
    error_count: u32,
}
```

**When**: `supervisor_strategy`メソッドをオーバーライドする

```rust
impl Actor for ResilientWorker {
    fn supervisor_strategy(&self, _ctx: &ActorContext) -> Option<SupervisorStrategy> {
        if self.error_count > 10 {
            Some(SupervisorStrategy::new(
                SupervisorStrategyKind::OneForOne,
                0,
                Duration::from_secs(1),
                |_| SupervisorDirective::Stop
            ))
        } else {
            None
        }
    }
}
```

**Then**:
- エラーカウントが10以下の場合、`None`が返されPropsのデフォルト戦略が使用される
- エラーカウントが10を超える場合、即座に停止する戦略が使用される

#### Scenario: デフォルト実装を使用する

**Given**: Actor実装が`supervisor_strategy`をオーバーライドしない

```rust
struct SimpleActor;

impl Actor for SimpleActor {
    fn receive(&mut self, ctx: &mut ActorContext, message: AnyMessageView) -> Result<(), ActorError> {
        // メッセージ処理
        Ok(())
    }
}
```

**When**: 子アクターが失敗する

**Then**:
- `supervisor_strategy`メソッドのデフォルト実装が`None`を返す
- Propsで指定されたデフォルト戦略が使用される

### Requirement: ActorCellによる動的戦略取得 (REQ-002)

ActorCellは子アクターの失敗時に、親Actor実装から監督戦略を動的に取得しなければならない (MUST)。

**優先度**: HIGH

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

#### Scenario: Actor実装から戦略を取得

**Given**:
- ActorCellが子アクターを管理している
- 親Actor実装が`supervisor_strategy`メソッドで`Some(strategy)`を返す

**When**: 子アクターが失敗する

**Then**:
1. `handle_failure`メソッドが呼び出される
2. `actor.lock()`で親Actor実装を取得
3. `actor.supervisor_strategy(ctx)`を呼び出す
4. 返された`Some(strategy)`を使用して失敗を処理
5. 適切な`SupervisorDirective`が決定される

#### Scenario: デフォルト戦略にフォールバック

**Given**:
- ActorCellが子アクターを管理している
- 親Actor実装が`supervisor_strategy`メソッドで`None`を返す
- ActorCellの`default_supervisor`フィールドにPropsから取得した戦略が保存されている

**When**: 子アクターが失敗する

**Then**:
1. `handle_failure`メソッドが呼び出される
2. `actor.supervisor_strategy(ctx)`が`None`を返す
3. `default_supervisor`フィールドの戦略を使用
4. 適切な`SupervisorDirective`が決定される

### Requirement: 優先順位ポリシー (REQ-003)

監督戦略の決定は、Actor実装による提供を優先し、提供されない場合はPropsのデフォルトにフォールバックしなければならない (MUST)。

**優先度**: MEDIUM

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

#### Scenario: 優先順位の適用

**Given**:
- Propsで`SupervisorStrategy::restarting(3, Duration::from_secs(10))`が指定されている
- Actor実装が状態に応じて異なる戦略を返す

**When**: 以下の状態で子アクターが失敗する
1. Actor実装が`Some(SupervisorStrategy::stopping())`を返す
2. Actor実装が`None`を返す

**Then**:
1. ケース1: 停止戦略が使用される（Actor実装の戦略が優先）
2. ケース2: 再起動戦略（3回、10秒以内）が使用される（Propsのデフォルトにフォールバック）

### Requirement: ActorCellのデフォルト戦略保持 (REQ-004)

ActorCellはProps由来のデフォルト監督戦略を`default_supervisor`フィールドに保持しなければならない (MUST)。

**優先度**: HIGH

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

#### Scenario: ActorCell生成時のデフォルト戦略保存

**Given**: Propsがカスタム監督戦略を持つ

```rust
let props = Props::from_fn(MyActor::new)
    .with_supervisor(SupervisorOptions::new(
        SupervisorStrategy::new(
            SupervisorStrategyKind::AllForOne,
            5,
            Duration::from_secs(30),
            |_| SupervisorDirective::Restart
        )
    ));
```

**When**: ActorCellが生成される

**Then**:
- `ActorCell::new`メソッドで`props.supervisor().strategy()`を呼び出す
- 取得した戦略を`default_supervisor`フィールドにコピーして保存
- このデフォルト戦略は後のフォールバックで使用される

### Requirement: 後方互換性 (REQ-005)

既存のActor実装は変更なしで動作し続けなければならない (MUST)。

**優先度**: CRITICAL

**適用範囲**:
- すべてのActorモジュール

#### Scenario: 既存Actor実装の動作継続

**Given**:
- 既存のActor実装が`supervisor_strategy`メソッドをオーバーライドしていない
- Propsでデフォルト戦略が指定されている

**When**: システムが動作する

**Then**:
- `supervisor_strategy`メソッドのデフォルト実装が`None`を返す
- Propsのデフォルト戦略が使用される
- 既存の動作が維持される

#### Scenario: 既存テストの継続性

**Given**: `modules/actor-core/tests/supervisor.rs`に既存のテストが存在

**When**: 変更を適用する

**Then**:
- すべての既存テストが引き続きパスする
- 特に`escalate_failure_restarts_supervisor`テストが正常に動作

### Requirement: パフォーマンス影響の最小化 (REQ-006)

失敗処理のパフォーマンスオーバーヘッドは最小限に抑えられなければならない (SHALL)。

**優先度**: MEDIUM

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

#### Scenario: 失敗処理時の追加コスト

**Given**: 子アクターが失敗する

**When**: `handle_failure`メソッドが実行される

**Then**:
- 追加コストはMutexロック1回とメソッド呼び出し1回のみ
- メッセージ処理パスには影響なし
- ActorCellのメモリ使用量増加は1フィールド分（約48バイト）のみ

### Requirement: エラーハンドリング (REQ-007)

`supervisor_strategy`メソッド内での例外発生には適切に対処しなければならない (MUST)。

**優先度**: MEDIUM

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

#### Scenario: supervisor_strategyメソッドでのパニック

**Given**: Actor実装の`supervisor_strategy`メソッドがパニックする

```rust
impl Actor for BuggyActor {
    fn supervisor_strategy(&self, _ctx: &ActorContext) -> Option<SupervisorStrategy> {
        panic!("Bug in strategy logic!");
    }
}
```

**When**: 子アクターが失敗する

**Then**:
- Mutexがpoisonedになる可能性があるが、システムは停止しない
- デフォルト戦略にフォールバックする、または親アクターが停止する
- システム全体がクラッシュしない

## 受入基準

以下のすべてが満たされること:

1. ✅ `Actor` traitに`supervisor_strategy`メソッドが追加され、デフォルト実装が`None`を返す
2. ✅ `ActorCell`が`default_supervisor`フィールドを持ち、Props由来の戦略を保存する
3. ✅ `handle_failure`メソッドがActor実装から戦略を動的に取得する
4. ✅ 優先順位ポリシー（Actor実装 → Propsデフォルト）が正しく実装されている
5. ✅ 既存のActor実装が変更なしで動作する
6. ✅ すべての既存テストがパスする
7. ✅ 新しいテストケースが追加され、動的戦略変更を検証する
8. ✅ RustDocドキュメントが充実している
9. ✅ パフォーマンス劣化が許容範囲（失敗処理時の軽微な追加コストのみ）
10. ✅ CIがすべてパスする

## 非機能要件

### パフォーマンス

- メッセージ処理パスには影響なし
- 失敗処理時の追加コスト: Mutexロック1回 + メソッド呼び出し1回
- メモリ増加: ActorCell当たり約48バイト

### 保守性

- コードの複雑度増加が最小限
- デバッグが容易（明示的な戦略選択ロジック）

### テスト容易性

- 戦略の動的変更をユニットテストで検証可能
- モックや状態操作でテストしやすい設計

## 制限事項

### 現在のスコープ外

- Typed ActorのBehaviors.supervise DSL（将来の拡張として検討）
- SupervisorStrategyの`Copy`制約削除（クロージャサポート）

### 既知の制約

- Actor実装が`supervisor_strategy`内でパニックした場合、デフォルト戦略へのフォールバックが保証されない可能性がある
- 再帰的な失敗シナリオでのスタックオーバーフロー防止は既存メカニズムに依存

## 依存関係

### 前提条件

- 既存のsupervision機構が正常に動作していること
- `Props`と`SupervisorOptions`が正しく実装されていること

### 影響を受けるコンポーネント

- `Actor` trait
- `ActorCell`
- `ActorContext` (コンテキスト生成ヘルパーが必要な場合)

## 参考資料

- [Pekko Fault Tolerance](https://pekko.apache.org/docs/pekko/current/fault-tolerance.html)
- Pekko Classic: `org.apache.pekko.actor.Actor#supervisorStrategy`
- 参照実装: `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:589`
