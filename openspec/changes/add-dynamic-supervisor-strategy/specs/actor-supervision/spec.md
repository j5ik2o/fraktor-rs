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
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> Option<SupervisorStrategy> {
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
2. `actor.lock()`で親Actor実装への可変参照を取得
3. `actor.supervisor_strategy(&mut ctx)`を呼び出す（状態更新可能）
4. 返された`Some(strategy)`を使用して失敗を処理
5. 適切な`SupervisorDirective`が決定される

#### Scenario: デフォルト戦略の使用

**Given**:
- ActorCellが子アクターを管理している
- 親Actor実装が`supervisor_strategy`メソッドをオーバーライドしていない

**When**: 子アクターが失敗する

**Then**:
1. `handle_failure`メソッドが呼び出される
2. `actor.supervisor_strategy(ctx)`がデフォルト実装により`SupervisorStrategy::default()`を返す
3. デフォルト戦略（OneForOne, 10回再起動, 1分以内）が使用される
4. 適切な`SupervisorDirective`が決定される

### Requirement: 後方互換性 (REQ-003)

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

### Requirement: パフォーマンス影響の最小化 (REQ-004)

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
- ActorCellのメモリ使用量が1フィールド分削減（約48バイト削減）

### Requirement: panic-free実装 (REQ-005)

`supervisor_strategy`メソッド実装はpanic-freeでなければならない (MUST)。

**優先度**: CRITICAL

**適用範囲**:
- `modules/actor-core/src/actor_prim/actor.rs`
- すべてのActor実装

**制約**:
- `actor-core`クレートは`#![no_std]`環境をサポート
- no_std環境ではpanic回復メカニズム（`catch_unwind`等）が利用できない
- `supervisor_strategy`内でパニックが発生した場合、ライブラリは関与しない
- パニックはアプリケーション全体の異常終了を引き起こす可能性がある

#### Scenario: panic-free実装の推奨

**Given**: Actor実装が`supervisor_strategy`をオーバーライドする

```rust
impl Actor for SafeActor {
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> Option<SupervisorStrategy> {
        // ✅ 良い例: panic-free実装
        if self.error_count > 10 {
            Some(SupervisorStrategy::stopping())
        } else {
            None
        }
    }
}
```

**When**: 子アクターが失敗する

**Then**:
- メソッドは正常に完了する
- システムは予測可能に動作する

#### Scenario: panic発生時の動作（非推奨）

**Given**: Actor実装の`supervisor_strategy`がパニックする

```rust
impl Actor for BuggyActor {
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> Option<SupervisorStrategy> {
        panic!("Bug in strategy logic!");  // ❌ 避けるべき
    }
}
```

**When**: 子アクターが失敗する

**Then**:
- **no_std環境**: アプリケーション全体が異常終了する（`panic = abort`）
- **std環境（panic = unwind）**: Mutexがpoisonedになり、以降のロック取得が失敗する
- いずれの場合も、ライブラリは関与せず、アプリケーション側の責任となる

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
