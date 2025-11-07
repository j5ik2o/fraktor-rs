# 実装タスク: 動的SupervisorStrategy取得機構

**Change ID**: `add-dynamic-supervisor-strategy`

## 実装チェックリスト

### フェーズ1: Actorトレイト拡張

- [ ] **actor-core**: `Actor` traitに`supervisor_strategy`メソッド追加
  - ファイル: `modules/actor-core/src/actor_prim/actor.rs`
  - デフォルト実装で`None`を返す
  - RustDocコメント追加（使用例、注意事項を含む）

- [ ] **actor-std**: stdモジュールの`Actor` traitも同様に拡張
  - ファイル: `modules/actor-std/src/actor_prim/actor.rs`
  - coreモジュールと同じシグネチャ

### フェーズ2: ActorCell/Props変更

- [ ] **ActorCell構造体**: `supervisor`フィールド削除
  - ファイル: `modules/actor-core/src/actor_prim/actor_cell.rs`
  - Props由来の固定戦略フィールドを削除

- [ ] **ActorCellコンストラクタ**: Props由来のsupervisor初期化を削除
  - `new`メソッドから`props.supervisor().strategy()`取得処理を削除
  - `supervisor`フィールド設定を削除

- [ ] **Props構造体**: `supervisor`フィールド削除
  - ファイル: `modules/actor-core/src/props/base.rs`
  - `PropsGeneric`から`supervisor: SupervisorOptions`を削除
  - `with_supervisor()`メソッド削除
  - `supervisor()`アクセサメソッド削除

- [ ] **handle_failureメソッド**: 動的戦略取得ロジック実装
  - `actor.lock()`でActor実装を取得
  - `actor.supervisor_strategy(&mut ctx)`を呼び出し
  - 返されたSupervisorStrategyを直接使用
  - 戦略に基づいて`SupervisorDirective`を決定

### フェーズ3: テスト追加

- [ ] **動的戦略変更テスト**: Actor状態に基づく戦略切り替えを確認
  - ファイル: `modules/actor-core/tests/supervisor.rs`
  - テストシナリオ:
    - Actor内部状態を変更して戦略が切り替わる
    - カスタム戦略を返すパターン
    - デフォルト実装でSupervisorStrategy::default()を返すパターン

- [ ] **Props経由の戦略指定テスト削除**
  - `.with_supervisor()`を使用する既存テストを削除または修正
  - Actor実装で`supervisor_strategy`をオーバーライドする形に書き換え

- [ ] **OneForOne/AllForOne動的切り替えテスト**
  - Actor状態に応じて戦略種別が変わることを確認

- [ ] **Escalate動作テスト**
  - Actor実装が`Escalate`を返す戦略を提供した場合の動作確認

- [ ] **既存テストの回帰確認**
  - `modules/actor-core/tests/supervisor.rs`の既存テストが継続して動作
  - `escalate_failure_restarts_supervisor`など

- [ ] **エッジケーステスト**
  - `supervisor_strategy`メソッド内で例外が発生した場合
  - 再帰的失敗のシナリオ

### フェーズ4: ドキュメント・サンプル

- [ ] **RustDoc更新**: `Actor` traitのドキュメント充実化
  - 使用例を複数追加
  - ユースケースの説明

- [ ] **サンプル実装**: examplesディレクトリに追加（オプション）
  - ファイル: `modules/actor-std/examples/dynamic_supervisor/main.rs`
  - エラーカウントに基づく戦略変更のデモ
  - ビジネスロジック状態に基づく判断のデモ

- [ ] **既存サンプルの更新**: supervision_std exampleの確認
  - ファイル: `modules/actor-std/examples/supervision_std/main.rs`
  - 既存のサンプルが引き続き動作することを確認

### フェーズ5: コード品質

- [ ] **Lint確認**: `cargo clippy`がパス
  - 新しいコードに対して警告なし

- [ ] **フォーマット**: `cargo fmt`実行
  - コードスタイル統一

- [ ] **ドキュメント**: `cargo doc`でドキュメント生成確認
  - 警告なし
  - リンク切れなし

- [ ] **CI確認**: `./scripts/ci-check.sh all`実行
  - すべてのチェックがパス

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

- [ ] すべてのテストがパス
- [ ] CIがグリーン
- [ ] ドキュメントが充実
- [ ] 破壊的変更なし（既存コードが動作）
- [ ] パフォーマンス劣化なし

## 注意事項

### 実装時の注意

1. **Actorロックの保持時間を最小化**
   - `supervisor_strategy`呼び出し前後でロックを取得・解放
   - デッドロックを避けるため、ネストしたロック取得は避ける

2. **Panic対策**
   - `supervisor_strategy`メソッド内でpanicが発生してもシステムが停止しないよう、適切なエラーハンドリング

3. **テストの網羅性**
   - 正常系だけでなく、異常系・エッジケースもカバー

### コミット戦略

- Phase単位でコミット
- 各Phaseが完了した時点でレビュー可能な状態にする
- コミットメッセージは明確に（例: `feat: add Actor::supervisor_strategy method`）

## レビューポイント

### コードレビュー時の確認事項

- [ ] Actor traitのシグネチャが適切
- [ ] ActorCellの変更が最小限
- [ ] テストカバレッジが十分
- [ ] ドキュメントが分かりやすい
- [ ] パフォーマンス影響が許容範囲
- [ ] エラーハンドリングが適切
- [ ] Pekko互換性が保たれている

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
