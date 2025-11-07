# 実装タスク: 動的SupervisorStrategy取得機構

**Change ID**: `add-dynamic-supervisor-strategy`

## 実装チェックリスト

### フェーズ1: Actorトレイト拡張

- [x] **actor-core**: `Actor` traitに`supervisor_strategy`メソッド追加
  - ファイル: `modules/actor-core/src/actor_prim/actor.rs`
  - デフォルト実装で`SupervisorStrategy::default()`を返す
  - RustDocコメント追加（使用例、注意事項を含む）

- [x] **actor-std**: stdモジュールの`Actor` traitも同様に拡張
  - ファイル: `modules/actor-std/src/actor_prim/actor.rs`
  - coreモジュールと同じシグネチャ

### フェーズ2: ActorCell/Props変更

- [x] **ActorCell構造体**: `supervisor`フィールド削除
  - ファイル: `modules/actor-core/src/actor_prim/actor_cell.rs`
  - Props由来の固定戦略フィールドを削除

- [x] **ActorCellコンストラクタ**: Props由来のsupervisor初期化を削除
  - `new`メソッドから`props.supervisor().strategy()`取得処理を削除
  - `supervisor`フィールド設定を削除

- [x] **Props構造体（core/std両方）**: `supervisor`フィールド削除
  - ファイル: `modules/actor-core/src/props/base.rs`, `modules/actor-std/src/props/base.rs`
  - `PropsGeneric`から`supervisor: SupervisorOptions`を削除
  - `with_supervisor()`/`supervisor()` APIと関連ドキュメントを削除

- [x] **handle_failureメソッド**: 動的戦略取得ロジック実装
  - `actor.lock()`でActor実装を取得
  - `actor.supervisor_strategy(&mut ctx)`を呼び出し
  - 返された`SupervisorStrategy`を直接使用
  - 戦略に基づいて`SupervisorDirective`を決定（`Clone`前提で扱う）

- [x] **SupervisorStrategy実装**: `Copy`制約を削除
  - ファイル: `modules/actor-core/src/supervision/base.rs`
  - すべての呼び出し元で`clone()`へ置き換え、ベンチ/サイズ計測を更新

- [x] **デフォルト戦略の実装**
  - `SupervisorStrategy`に`impl Default`/`fn default()`を追加（OneForOne, 10回, 1秒, Recoverable→Restart/Fatal→Stop）
  - `SupervisorOptions::default()`は新しい`SupervisorStrategy::default()`を委譲するのみとし、既存挙動を維持

### フェーズ3: テスト追加

- [x] **動的戦略変更テスト**: Actor状態に基づく戦略切り替えを確認
  - ファイル: `modules/actor-core/tests/supervisor.rs`
  - テストシナリオ:
    - Actor内部状態を変更して戦略が切り替わる
    - カスタム戦略を返すパターン
    - デフォルト実装でSupervisorStrategy::default()を返すパターン

- [x] **Props経由の戦略指定テスト削除**
  - `.with_supervisor()`を使用する既存テストを削除または修正
  - Actor実装で`supervisor_strategy`をオーバーライドする形に書き換え

- [x] **OneForOne/AllForOne動的切り替えテスト**
  - Actor状態に応じて戦略種別が変わることを確認

- [x] **Escalate動作テスト**
  - Actor実装が`Escalate`を返す戦略を提供した場合の動作確認

- [x] **既存テストの回帰確認**
  - `modules/actor-core/tests/supervisor.rs`の既存テストが継続して動作
  - `escalate_failure_restarts_supervisor`など - **Box<T>転送メソッドバグを修正**

- [x] **actor-std受け入れテスト**
  - ファイル: `modules/actor-std/tests/tokio_acceptance.rs`
  - `.with_supervisor()`削除後もシナリオが通ることを確認

- [x] **エッジケーステスト**
  - `supervisor_strategy`がpanic-freeであることをドキュメント化し、panic発生時にライブラリがフォールバックしないことを確認
  - 再帰的失敗のシナリオ

- [x] **デフォルト戦略テスト**
  - `SupervisorStrategy::default()`がRecoverable→Restart/Fatal→Stopを返すこと、および監視ウィンドウ(1秒)/最大再起動回数(10回)を満たすことを検証

### フェーズ4: ドキュメント・サンプル

- [x] **RustDoc更新**: `Actor` traitのドキュメント充実化
  - 使用例を複数追加
  - ユースケースの説明

- [x] **サンプル実装**: examplesディレクトリに追加（オプション）
  - ファイル: `modules/actor-std/examples/supervision_std/main.rs`
  - エラーカウントに基づく戦略変更のデモ（既存サンプル修正で対応）

- [x] **既存サンプルの更新**: supervision_std exampleの確認
  - ファイル: `modules/actor-std/examples/supervision_std/main.rs`
  - 既存のサンプルが引き続き動作することを確認

- [x] **移行ドキュメント**
  - ファイル: `CHANGELOG.md`, `claudedocs/migration_dynamic_supervisor_strategy.md`
  - BREAKING CHANGEとBefore/Afterコードを追加

### フェーズ5: コード品質

- [x] **Lint確認**: `cargo clippy`がパス
  - 新しいコードに対して警告なし

- [x] **フォーマット**: `cargo fmt`実行
  - コードスタイル統一

- [x] **ドキュメント**: `cargo doc`でドキュメント生成確認
  - 警告なし
  - リンク切れなし

- [x] **CI確認**: `./scripts/ci-check.sh all`実行
  - すべてのチェックがパス (全218テスト + tokio 3テスト成功)

## 実装順序

1. **Phase 1** → Actor traitの拡張（最も基礎的な変更）
2. **Phase 2** → ActorCellの変更（コア機能実装）
3. **Phase 3** → テスト（機能検証）
4. **Phase 4** → ドキュメント（使用方法の明確化）
5. **Phase 5** → コード品質（最終チェック）

## 依存関係

```
Phase 1 (Actor trait)
    ↓
Phase 2 (ActorCell)
    ↓
Phase 3 (Tests)
    ↓
Phase 4 (Docs)
    ↓
Phase 5 (Quality)
```

## 完了条件

すべてのチェックボックスが `[x]` になり、以下が確認されること:

- [x] すべてのテストがパス (actor-core: 218テスト, tokio: 3テスト)
- [x] CIがグリーン (cargo fmt, clippy, test, doc すべて成功)
- [x] ドキュメントが充実 (RustDoc, CHANGELOG, 移行ガイド完備)
- [x] 破壊的変更はすべて移行ガイドとCHANGELOGで周知済み
- [x] パフォーマンス劣化なし (動的戦略取得のみ追加、既存動作は変更なし)

## 注意事項

### 実装時の注意

1. **Actorロックの保持時間を最小化**
   - `supervisor_strategy`呼び出し前後でロックを取得・解放
   - デッドロックを避けるため、ネストしたロック取得は避ける

2. **Panic対策**
   - `supervisor_strategy`はpanic-freeに実装することをRustDoc/テストで保証（ライブラリ側でpanicを握り潰さない）

3. **テストの網羅性**
   - 正常系だけでなく、異常系・エッジケースもカバー

### コミット戦略

- Phase単位でコミット
- 各Phaseが完了した時点でレビュー可能な状態にする
- コミットメッセージは明確に（例: `feat: add Actor::supervisor_strategy method`）

## レビューポイント

### コードレビュー時の確認事項

- [x] Actor traitのシグネチャが適切
- [x] ActorCellの変更が最小限
- [x] テストカバレッジが十分
- [x] ドキュメントが分かりやすい
- [x] パフォーマンス影響が許容範囲
- [x] エラーハンドリングが適切
- [x] Pekko互換性が保たれている

### 実装完了 (2025-11-07)

すべてのフェーズ (1-5) が完了し、全テストが成功しました。

**重要なバグ修正:**
- `Box<T>` の `Actor` トレイト実装に `supervisor_strategy()` 転送メソッドが欠けていた問題を修正
- これにより、trait objectとして保存されたアクターでも正しくカスタム監督戦略が適用されるようになりました

**テスト結果:**
- actor-core: 218テスト成功
- tokio統合: 3テスト成功
- すべてのコンパイルエラー解決
- cargo fmt, clippy, doc すべて成功

## 参考実装

### Pekko Classic

```scala
// Actor.scala:589
def supervisorStrategy: SupervisorStrategy = SupervisorStrategy.defaultStrategy
```

### 既存のsupervisor tests

```rust
// modules/actor-core/tests/supervisor.rs
#[test]
fn escalate_failure_restarts_supervisor() {
  // ...
}
```
