# 提案: 動的SupervisorStrategy取得機構

**Change ID**: `add-dynamic-supervisor-strategy`
**作成日**: 2025-11-07
**ステータス**: 提案中

## 概要

Actorトレイトに`supervisor_strategy`メソッドを追加し、Actor実装の内部状態に基づいた動的な監督戦略の決定を可能にする。

## 動機

### 現状の問題

1. **SupervisorStrategyが生成時に固定**
   - `Props`経由で設定されたSupervisorStrategyは`ActorCell`生成時にコピーされる
   - 実行時の状態変化に応じた監督方針の変更ができない

2. **Actor状態へのアクセス不可**
   - SupervisorStrategyのdecider関数はエラー情報のみで判断
   - Actor内部状態（エラーカウント、モード設定など）を参照できない

3. **Pekko untypedとの互換性**
   - Pekko Classicでは`supervisorStrategy`メソッドをオーバーライド可能
   - Actor実装レベルで監督戦略を定義できる

### 具体的なユースケース

```rust
// ユースケース1: エラーカウントに基づく厳格モード
struct ResilientWorker {
    consecutive_errors: u32,
}

impl Actor for ResilientWorker {
    fn supervisor_strategy(&self, _ctx: &ActorContext) -> Option<SupervisorStrategy> {
        if self.consecutive_errors > 10 {
            // 10回以上連続エラー → 即座に停止
            Some(SupervisorStrategy::stopping())
        } else {
            // 通常時 → 3回まで再試行
            Some(SupervisorStrategy::restarting(3, Duration::from_secs(10)))
        }
    }
}

// ユースケース2: ビジネスロジックの状態に基づく判断
struct PaymentProcessor {
    critical_mode: bool, // 決済処理中など
}

impl Actor for PaymentProcessor {
    fn supervisor_strategy(&self, _ctx: &ActorContext) -> Option<SupervisorStrategy> {
        if self.critical_mode {
            // クリティカルな処理中はエスカレート
            Some(SupervisorStrategy::escalating())
        } else {
            None // デフォルト戦略を使用
        }
    }
}
```

## 提案内容

### 1. Actorトレイトの拡張

`Actor` traitに新しいメソッドを追加:

```rust
pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send {
    // 既存のメソッド...

    /// 子アクターの監督戦略を提供する。
    ///
    /// # デフォルト実装
    ///
    /// `None`を返し、`Props`で指定された戦略を使用する。
    ///
    /// # カスタマイズ
    ///
    /// Actor内部の状態に基づいて動的に戦略を決定できる。
    ///
    /// # 例
    ///
    /// ```rust
    /// fn supervisor_strategy(&self, ctx: &ActorContext) -> Option<SupervisorStrategy> {
    ///     if self.strict_mode {
    ///         Some(SupervisorStrategy::stopping())
    ///     } else {
    ///         Some(SupervisorStrategy::restarting(5, Duration::from_secs(10)))
    ///     }
    /// }
    /// ```
    fn supervisor_strategy(&self, _ctx: &ActorContextGeneric<'_, TB>) -> Option<SupervisorStrategy> {
        None
    }
}
```

### 2. ActorCellの変更

`handle_failure`メソッドで監督戦略を動的に取得:

```rust
// 現在の実装（actor_cell.rs:421）
let directive = {
    let mut stats = self.child_stats.lock();
    let entry = find_or_insert_stats(&mut stats, child);
    self.supervisor.handle_failure(entry, error, now)  // 固定されたsupervisor
};

// 変更後
let directive = {
    let mut stats = self.child_stats.lock();
    let entry = find_or_insert_stats(&mut stats, child);

    // Actor実装から戦略を取得、なければPropsのデフォルトを使用
    let strategy = {
        let actor = self.actor.lock();
        actor.supervisor_strategy(&ctx)
            .unwrap_or(self.default_supervisor)
    };

    strategy.handle_failure(entry, error, now)
};
```

### 3. 優先順位ポリシー

監督戦略の決定優先順位:

1. **Actor実装の`supervisor_strategy`メソッド** （最優先）
   - `Some(strategy)`を返した場合、その戦略を使用
2. **Propsで指定された戦略**
   - Actor実装が`None`を返した場合のフォールバック

### 4. SupervisorStrategyの制約緩和

現在`Copy` traitを要求しているが、これを緩和する可能性を検討:

```rust
// 現在: Copy制約あり
#[derive(Clone, Copy, Debug)]
pub struct SupervisorStrategy { ... }

// 提案: Clone のみでOK（クロージャサポートのため）
#[derive(Clone, Debug)]
pub struct SupervisorStrategy { ... }
```

**理由**: 将来的にクロージャベースのdeciderをサポートする場合、Copyでは制限が厳しい。

## 影響範囲

### 変更が必要なファイル

1. **modules/actor-core/src/actor_prim/actor.rs**
   - `Actor` traitに`supervisor_strategy`メソッド追加

2. **modules/actor-core/src/actor_prim/actor_cell.rs**
   - `ActorCell`構造体に`default_supervisor`フィールド追加
   - `handle_failure`メソッドで動的戦略取得ロジック実装

3. **modules/actor-std/src/actor_prim/actor.rs**
   - stdモジュールの`Actor` traitも同様に拡張

4. **modules/actor-core/tests/supervisor.rs**
   - 動的戦略変更のテストケース追加

5. **modules/actor-core/src/supervision/base.rs**
   - `SupervisorStrategy`の`Copy`制約削除を検討

### 破壊的変更

**軽微な破壊的変更**:

- `SupervisorStrategy`から`Copy` traitを削除する場合
  - 影響: 構造体のコピーが必要な箇所で`clone()`を明示的に呼ぶ必要がある
  - 緩和策: 当面は`Copy`を維持し、将来的な拡張として検討

**非破壊的変更**:

- `Actor` traitへのデフォルト実装付きメソッド追加
  - 既存のActor実装は変更不要（デフォルトで`None`を返す）

## 設計判断

### 判断1: Actor参照の渡し方

**選択肢**:

1. **`&self`で直接アクセス** ✅ 採用
   - `ActorCell`が`self.actor.lock()`でActor実装を取得
   - 型安全で明示的

2. **`&dyn Any`で渡す**
   - Actor実装を`Any`として渡し、downcastが必要
   - 柔軟だが型安全性が低い

3. **Contextから間接的に取得**
   - `ActorContext`に`actor()`メソッドを追加
   - 複雑でメリットが少ない

**理由**: シンプルで型安全な選択肢1を採用。

### 判断2: デフォルト戦略の保存場所

`ActorCell`に`default_supervisor: SupervisorStrategy`フィールドを追加し、Props由来の戦略を保存。

**理由**:
- Actor実装が`None`を返した際のフォールバックとして必要
- Props生成時の戦略を維持

### 判断3: Copy制約の扱い

**当面は維持、将来的に削除を検討**

**理由**:
- 現時点でクロージャdeciderの需要は不明
- 破壊的変更のリスクを最小化
- 必要になった時点で別のOpenSpecとして提案

## 実装フェーズ

### フェーズ1: Actorトレイト拡張
- [ ] `actor_prim/actor.rs`に`supervisor_strategy`メソッド追加
- [ ] stdモジュールも同様に拡張
- [ ] RustDocコメント追加

### フェーズ2: ActorCell変更
- [ ] `ActorCell`に`default_supervisor`フィールド追加
- [ ] コンストラクタで`Props`から戦略をコピー
- [ ] `handle_failure`で動的戦略取得ロジック実装

### フェーズ3: テスト追加
- [ ] 動的戦略変更のテストケース作成
- [ ] エッジケース（Escalate、再帰的失敗）のテスト
- [ ] 既存のsupervisorテストが継続動作することを確認

### フェーズ4: ドキュメント更新
- [ ] Actorトレイトの使用例をRustDocに追加
- [ ] ユースケースをexamplesディレクトリに追加

## セキュリティ・パフォーマンス考慮事項

### パフォーマンス

- **影響**: 軽微
  - `handle_failure`時に1回の`actor.lock()`とメソッド呼び出しが追加
  - 失敗処理は頻繁に発生しないため、ホットパスではない

### メモリ使用量

- **影響**: 最小限
  - `ActorCell`に1フィールド（`SupervisorStrategy`）追加
  - `SupervisorStrategy`は小さい構造体（数十バイト）

### スレッドセーフティ

- **影響**: なし
  - Actor実装は既に`Send`を要求
  - `ActorCell::actor`は既に`Mutex`で保護されている

## 代替案

### 代替案1: Props経由のクロージャ

```rust
let props = Props::from_fn(MyActor::new)
    .with_supervisor_factory(|actor: &MyActor, error| {
        // actorを参照して戦略を決定
    });
```

**却下理由**:
- Actor型を`Props`が知る必要があり、型消去と相性が悪い
- Pekko互換性がない

### 代替案2: メッセージ経由の戦略変更

```rust
actor_ref.tell(UpdateSupervisorStrategy::new(strategy));
```

**却下理由**:
- 戦略がメッセージキューに入るため、適用タイミングが不明確
- 競合状態のリスク

## 将来の拡張可能性

### Typed Actorへの対応

本提案はUntypedレイヤーのみをスコープとするが、将来的に:

1. **Behaviors.supervise DSL**
   - Typedレイヤーで`Behaviors::supervise(behavior).on_failure(...)`を実装
   - Behaviorラッパー方式で監督戦略を宣言的に指定

2. **共存設計**
   - Untyped: Actor実装レベルの戦略（本提案）
   - Typed: Behaviorレベルの戦略（将来）
   - レイヤーが異なるため競合しない

## 承認基準

以下の条件を満たすこと:

- [ ] 設計レビュー完了
- [ ] 破壊的変更の影響範囲が明確
- [ ] テスト計画が妥当
- [ ] Pekko互換性が確認されている
- [ ] パフォーマンス影響が許容範囲

## 参考資料

- Pekko Classic: `org.apache.pekko.actor.Actor#supervisorStrategy`
- 参照実装: `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:589`
- ドキュメント: https://pekko.apache.org/docs/pekko/current/fault-tolerance.html
