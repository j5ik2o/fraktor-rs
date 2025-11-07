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
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
        if self.consecutive_errors > 10 {
            // 10回以上連続エラー → 即座に停止
            SupervisorStrategy::stopping()
        } else {
            // 通常時 → 3回まで再試行
            SupervisorStrategy::restarting(3, Duration::from_secs(10))
        }
    }
}

// ユースケース2: ビジネスロジックの状態に基づく判断
use cellactor::logging::LogLevel;

struct PaymentProcessor {
    critical_mode: bool, // 決済処理中など
}

impl Actor for PaymentProcessor {
    fn supervisor_strategy(&mut self, ctx: &mut ActorContext) -> SupervisorStrategy {
        if self.critical_mode {
            // クリティカルな処理中はエスカレート。Context経由でロガーを利用
            ctx.log(LogLevel::Warn, "escalate to payment guardian");
            SupervisorStrategy::escalating()
        } else {
            SupervisorStrategy::default()
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
    /// `SupervisorStrategy::default()`を返す。PropsやActorCellは監督戦略を保持しないため、
    /// このメソッドが唯一の情報源となる。
    ///
    /// # カスタマイズ
    ///
    /// Actor内部の状態に基づいて動的に戦略を決定できる。
    /// 必要に応じて`ctx`からシステム情報を取得し、状態更新も行える。
    ///
    /// # 例
    ///
    /// ```rust
    /// fn supervisor_strategy(&mut self, ctx: &mut ActorContext) -> SupervisorStrategy {
    ///     if self.strict_mode {
    ///         SupervisorStrategy::stopping()
    ///     } else {
    ///         SupervisorStrategy::default()
    ///     }
    /// }
    /// ```
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
        SupervisorStrategy::default()
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
    self.supervisor.handle_failure(entry, error, now)  // Props由来の固定戦略
};

// 変更後
let strategy = {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContextGeneric::new(&system, self.pid);
    let mut actor = self.actor.lock();
    actor.supervisor_strategy(&mut ctx)  // Actor実装から直接取得
};

let directive = {
    let mut stats = self.child_stats.lock();
    let entry = find_or_insert_stats(&mut stats, child);
    strategy.handle_failure(entry, error, now)
};
```

### 3. Props簡素化

**重要な設計変更**: PropsからSupervisorStrategy指定を削除

**理由**:
- **Pekko互換性**: Pekko Classicでは`Props`に`supervisorStrategy`フィールドなし
- **責務の明確化**: Props = インスタンス化設定、SupervisorStrategy = Actor振る舞い
- **設計の簡素化**: フィールド重複なし、優先順位ポリシー不要、フォールバック不要

```rust
// Props構造体から削除
pub struct PropsGeneric<TB: RuntimeToolbox + 'static> {
    factory:    ArcShared<dyn ActorFactory<TB>>,
    name:       Option<String>,
    mailbox:    MailboxConfig,
    // supervisor: SupervisorOptions,  // ← 削除
    middleware: Vec<String>,
    dispatcher: DispatcherConfigGeneric<TB>,
}

// ActorCell構造体からも削除
pub struct ActorCellGeneric<TB: RuntimeToolbox + 'static> {
    // ...
    // supervisor: SupervisorStrategy,  // ← 削除
    // ...
}
```

### 4. デフォルト戦略

Actor traitのデフォルト実装で`SupervisorStrategy::default()`を返し、PropsやActorCellが戦略を保持しない新しい責務分担に揃える。同時に`SupervisorStrategy`へ`Default`実装および`fn default() -> Self`を追加し、従来`SupervisorOptions::default()`が提供していた挙動（OneForOne / 10回 / 1秒 / Recoverable=Restart, Fatal=Stop）を完全に移植する。

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

pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send {
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
        SupervisorStrategy::default()
    }
}
```

`SupervisorOptions::default()`は互換性のために残しつつ、内部で`SupervisorStrategy::default()`を呼び出すだけの薄いラッパーに縮小する。

**再確認したデフォルト挙動**:
- 種類: `OneForOne`
- 最大再起動回数: 10 回
- 監視ウィンドウ: 1 秒
- Decider: `ActorError::Recoverable(_) => Restart` / `ActorError::Fatal(_) => Stop`
  - Escalationはユーザー実装の戦略でのみ選択される

### 5. SupervisorStrategyの制約緩和

`SupervisorStrategy`から`Copy`トレイトを即時に削除し、`Clone`のみに揃える。

```rust
#[derive(Clone, Debug)]
pub struct SupervisorStrategy { /* deciderクロージャを格納できる */ }
```

**理由**:
- `supervisor_strategy(&mut self, ..)`は毎回新しい`SupervisorStrategy`を生成するため、Copyによる暗黙コピーは不要
- deciderをクロージャで表現したい将来拡張（状態を捕捉するrestartポリシーなど）の足かせを事前に取り除ける
- Propsからのフォールバックを廃止したため、グローバルに共有される固定戦略の需要が低下

**影響**:
- これまで`Copy`に依存していた箇所（例: `strategy.handle_failure(...)`呼び出し前に一時変数へコピー）は`clone()`へ置き換える
- 公開APIの挙動は変わらず、破壊的変更は`SupervisorStrategy: Copy`を前提にしていた利用者のみ

## 影響範囲

### 変更が必要なファイル

1. **modules/actor-core/src/actor_prim/actor.rs**  
   - `Actor` traitに`fn supervisor_strategy(&mut self, &mut ActorContext) -> SupervisorStrategy`を追加し、RustDocでデフォルト実装・使用例を更新

2. **modules/actor-std/src/actor_prim/actor.rs**  
   - std版Actor traitも同じAPIへ揃え、ドキュメントを同期

3. **modules/actor-core/src/actor_prim/actor_cell.rs**  
   - 構造体・`create`・`handle_child_failure`・`handle_failure`から`supervisor`フィールド参照を削除し、Actorから直接戦略を取得するロジックを導入

4. **modules/actor-core/src/actor_prim/actor_cell/tests.rs**  
   - ActorCell単体テストで動的なOneForOne/AllForOne切り替え、Escalateフォールバックを検証

5. **modules/actor-core/src/props/base.rs**  
   - `PropsGeneric`から`supervisor: SupervisorOptions`を除去し、`with_supervisor`/`supervisor` APIと関連ドキュメントを削除

6. **modules/actor-std/src/props/base.rs**  
   - std向けPropsラッパーから同APIを削除し、ビルダー例をActorメソッド方式へ書き換え

7. **modules/actor-core/src/supervision/base.rs**  
   - `SupervisorStrategy`から`Copy`制約を削除し、`Clone`ベースの実装へ変更（deciderクロージャを保持できるよう準備）

8. **modules/actor-core/tests/supervisor.rs**  
   - `.with_supervisor()`に依存しているテストをActor実装オーバーライド方式へ移行し、新しいユースケースを追加

9. **modules/actor-std/tests/tokio_acceptance.rs**  
   - 受け入れテストのProps構築を修正し、API破壊的変更によるビルド失敗を防止

10. **modules/actor-std/examples/supervision_std/main.rs**  
    - ガイドで使用しているPropsビルダーの移行例を提供し、ユーザー向けのリファレンスを最新化

11. **CHANGELOG.md / docs/guides/**  
    - BREAKING CHANGEとして`Props::with_supervisor`削除と移行手順を周知

### 破壊的変更

**重大な破壊的変更**:

- **Props APIの変更**
  - `Props::with_supervisor()`および`Props::supervisor()`アクセサを削除
  - 影響: 既存コードで`.with_supervisor()`を使用している箇所はコンパイルエラー
  - 移行方法: Actor実装で`supervisor_strategy`メソッドをオーバーライド

- **SupervisorStrategyのCopy撤廃**
  - `SupervisorStrategy: Copy`に依存していたコードは`clone()`へ置き換える必要がある
  - `SupervisorStrategy::default()`は`Clone`前提で再生成できるため、大半のケースで追加コストは無視できる

**移行例**:
```rust
// 変更前
let props = Props::from_fn(MyActor::new)
    .with_supervisor(SupervisorOptions::new(
        SupervisorStrategy::stopping()
    ));

// 変更後
impl Actor for MyActor {
    fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
        SupervisorStrategy::stopping()
    }
}
let props = Props::from_fn(MyActor::new);
```

### 移行ガイド

1. **影響箇所の洗い出し**: `rg --context 2 "with_supervisor" -n modules` を実行してすべての呼び出し元を列挙する（2025-11-07時点で9箇所以上）。
2. **Actor実装へのロジック移動**: Propsで設定していた戦略をActor実装に移し、`supervisor_strategy`で返す。

   **AllForOne戦略の例**

   ```rust
   // 変更前
   let supervisor = SupervisorStrategy::all_for_one(5, Duration::from_secs(30), decider);
   let props = Props::from_fn(Guardian::new)
       .with_supervisor(SupervisorOptions::new(supervisor));

   // 変更後
   use cellactor::logging::LogLevel;

   impl Actor for Guardian {
       fn supervisor_strategy(&mut self, ctx: &mut ActorContext) -> SupervisorStrategy {
           ctx.log(LogLevel::Warn, "switch entire tree to strict supervision");
           SupervisorStrategy::all_for_one(5, Duration::from_secs(30), decider)
       }
   }
   let props = Props::from_fn(Guardian::new);
   ```

   **Escalate戦略の例**

   ```rust
   use cellactor::logging::LogLevel;

   impl Actor for EscalatingGuardian {
       fn supervisor_strategy(&mut self, ctx: &mut ActorContext) -> SupervisorStrategy {
           ctx.log(LogLevel::Warn, "escalate to higher guardian");
           SupervisorStrategy::escalating()
       }
   }
   ```

3. **テスト/サンプルの順次更新**:
   - `modules/actor-core/tests/supervisor.rs`: Props由来の設定をActor実装オーバーライドへ変換し、新しい動的シナリオを追加
   - `modules/actor-std/tests/tokio_acceptance.rs`: 受け入れテスト内で`.with_supervisor()`を削除
   - `modules/actor-std/examples/supervision_std/main.rs`: ドキュメントに掲載するBefore/Afterを整備
4. **ドキュメント告知**: `CHANGELOG.md`と`docs/guides/actor-system.md`にBREAKING CHANGEを明記し、上記Before/Afterを引用できるようにする。

**非破壊的変更**:

- `Actor` traitへのデフォルト実装付きメソッド追加
  - 既存のActor実装は変更不要（デフォルト戦略が自動適用）

## 設計判断

### 判断1: Actor参照の渡し方

**選択肢**:

1. **`&mut self`で直接アクセス** ✅ 採用
   - `ActorCell`が`self.actor.lock()`でActor実装への可変参照を取得
   - 型安全で明示的
   - 既存のActor traitメソッド（receive, pre_startなど）と一貫したシグネチャ
   - 状態更新が自然に可能

2. **`&dyn Any`で渡す**
   - Actor実装を`Any`として渡し、downcastが必要
   - 柔軟だが型安全性が低い

3. **Contextから間接的に取得**
   - `ActorContext`に`actor()`メソッドを追加
   - 複雑でメリットが少ない

**理由**: シンプルで型安全な選択肢1を採用。

### 判断2: Props vs Actor traitでの戦略指定

**Props削除、Actor trait のみで戦略を提供** ✅ 採用

**理由**:
- **Pekko互換性**: Pekko Classicは`Props`に`supervisorStrategy`フィールドなし
- **責務の明確化**: Props = インスタンス化設定、SupervisorStrategy = Actor振る舞い
- **設計の簡素化**: フィールド重複なし、優先順位ポリシー不要、フォールバック不要
- **メモリ効率**: ActorCellから`supervisor`フィールド削除で48バイト削減
- **実用性**: Props指定の正当なユースケースが見当たらない

**代替案として検討したが却下**:
- 両方サポート（Props + Actor trait）: 複雑すぎる、二重管理のバグ温床
- Props のみ: Actor状態に基づく動的変更ができない

### 判断3: Copy制約の扱い

**即時に削除し、Cloneのみを要求** ✅ 採用

**理由**:
- `supervisor_strategy`が毎回新しい値を返すため、`Copy`による暗黙コピーは不要
- クロージャ/状態付きdeciderを実現する際に`Copy`がボトルネックになる
- Propsからのフォールバックを廃止したため、共有参照として戦略を再利用する要件が薄れた

**移行策**:
- core/std両モジュールで`SupervisorStrategy`を複製していた箇所は`clone()`呼び出しに統一
- 破壊的変更はこの提案のフェーズ内でまとめてリリースし、追加のBreakingを避ける

### 判断4: panic処理の扱い

**panic-free実装を要求、ライブラリは関与しない** ✅ 採用

**理由**:
- `actor-core`クレートは`#![no_std]`環境をサポート
- no_std環境ではpanic回復メカニズム（`catch_unwind`等）が利用できない
- `supervisor_strategy`内でパニックが発生した場合:
  - **no_std環境（panic = abort）**: アプリケーション全体が異常終了
  - **std環境（panic = unwind）**: Mutexがpoisonedになり、システムが不安定化
- いずれの場合も、ライブラリは関与せず、アプリケーション側の責任
- Actor実装者は必ずpanic-freeな実装を提供すべき

## 実装フェーズ

### フェーズ1: Actorトレイト拡張
- [ ] `actor_prim/actor.rs`に`supervisor_strategy`メソッド追加
- [ ] stdモジュールも同様に拡張
- [ ] RustDocコメント追加

### フェーズ2: ActorCell/Props/戦略定義の更新
- [ ] `ActorCell`構造体から`supervisor`関連フィールドを完全削除
- [ ] コンストラクタから`Props`由来の戦略コピー処理を削除
- [ ] `PropsGeneric`から`supervisor`フィールドと`.with_supervisor()`/`.supervisor()` APIを除去
- [ ] `handle_failure`でActor実装から直接`SupervisorStrategy`を取得するロジックを実装
- [ ] `SupervisorStrategy`の`Copy`制約を外し、必要箇所を`clone()`に置き換える

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
  - 実装時に`cargo bench -p actor-core supervisor_failures`を追加し、旧ロジックと新ロジックの平均処理時間を比較する

### メモリ使用量

- **影響**: 削減
  - `ActorCell`から`supervisor`フィールドが消えるため、1セルあたり約48バイト節約
  - Props側の`SupervisorOptions`保有コストもゼロになる
  - `std::mem::size_of::<ActorCellGeneric<StdToolbox>>()`および`::<SupervisorStrategy>()`の実測値を記録し、ドキュメント/CHANGELOGに掲載する

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
