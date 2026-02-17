# fraktor-rs コーディングポリシー

## 原則

| 原則 | 基準 |
|------|------|
| Less is more / YAGNI | 要件達成に必要最低限の設計。「将来使うかも」は REJECT |
| 後方互換不要 | 破壊的変更を恐れず最適な設計を追求 |
| 一貫性 | 既存の実装パターンに従う。独自パターンの導入は REJECT |

## 構造ルール（Dylint lintで機械的強制）

以下の lint に違反する実装は REJECT:

| lint | 内容 |
|------|------|
| type-per-file | 1公開型 = 1ファイル |
| mod-file | mod.rsではなく型名.rsでモジュール定義 |
| module-wiring | モジュール配線の整合性 |
| tests-location | テストは `{name}/tests.rs` に配置 |
| use-placement | use文は関数内ではなくファイル先頭 |
| rustdoc | 公開型にはrustdoc（英語）必須 |
| cfg-std-forbid | coreモジュールでのstd依存禁止 |
| ambiguous-suffix | Manager/Util/Facade/Service/Runtime/Engine 禁止 |

## 可変性ポリシー

| ルール | 基準 |
|--------|------|
| 内部可変性 | デフォルト禁止。可変操作は `&mut self` で設計 |
| 共有型 | AShared パターンのみ許容（ArcShared + ToolboxMutex） |
| `&self` + 内部可変性 | 人間の許可なく使用は REJECT |

## CQS (Command-Query Separation)

| 種類 | シグネチャ |
|------|-----------|
| Query | `&self` + 戻り値あり |
| Command | `&mut self` + `()` or `Result<(), E>` |
| `&mut self` + 戻り値 | CQS違反。分離するか人間の許可が必要 |

## 命名規約

| 対象 | 規約 |
|------|------|
| ファイル | `snake_case.rs` |
| 型/trait | `PascalCase` |
| rustdoc | 英語 |
| コメント/Markdown | 日本語 |
| 禁止サフィックス | Manager, Util, Facade, Service, Runtime, Engine |

## Pekko参照実装からの変換ルール

| Pekko パターン | Rust パターン |
|----------------|--------------|
| `trait Actor` | `BehaviorGeneric<TB, M>` |
| `ActorRef[T]` | `TypedActorRefGeneric<TB, M>` |
| `implicit` | `TB: RuntimeToolbox` パラメータ |
| `sealed trait` + case classes | `enum` |
| `FiniteDuration` | `ticks: usize`（tickベースモデル） |

## テストポリシー

- 新規作成した型・関数には必ず単体テストを追加
- テストファイルは `{type_name}/tests.rs` に配置
- テスト実行は必須。実装完了後に `cargo test` で結果確認
- テストをコメントアウトしたり無視したりしない

## 禁止事項

- lint エラーを `#[allow]` で回避（人間の許可なし）
- `#![no_std]` の core モジュールで std 依存を導入
- 参照実装を読まずに独自設計を進める
- CHANGELOG.md の編集（GitHub Action が自動生成）


---

# テストポリシー

全ての振る舞いの変更には対応するテストが必要であり、全てのバグ修正にはリグレッションテストが必要。

## 原則

| 原則 | 基準 |
|------|------|
| Given-When-Then | テストは3段階で構造化する |
| 1テスト1概念 | 複数の関心事を1テストに混ぜない |
| 振る舞いを検証 | 実装の詳細ではなく振る舞いをテストする |
| 独立性 | 他のテストや実行順序に依存しない |
| 再現性 | 時間やランダム性に依存せず、毎回同じ結果 |

## カバレッジ基準

| 対象 | 基準 |
|------|------|
| 新しい振る舞い | テスト必須。テストがなければ REJECT |
| バグ修正 | リグレッションテスト必須。テストがなければ REJECT |
| 振る舞いの変更 | テストの更新必須。更新がなければ REJECT |
| エッジケース・境界値 | テスト推奨（Warning） |

## テスト優先度

| 優先度 | 対象 |
|--------|------|
| 高 | ビジネスロジック、状態遷移 |
| 中 | エッジケース、エラーハンドリング |
| 低 | 単純なCRUD、UIの見た目 |

## テスト構造: Given-When-Then

```typescript
test('ユーザーが存在しない場合、NotFoundエラーを返す', async () => {
  // Given: 存在しないユーザーID
  const nonExistentId = 'non-existent-id'

  // When: ユーザー取得を試みる
  const result = await getUser(nonExistentId)

  // Then: NotFoundエラーが返る
  expect(result.error).toBe('NOT_FOUND')
})
```

## テスト品質

| 観点 | 良い | 悪い |
|------|------|------|
| 独立性 | 他のテストに依存しない | 実行順序に依存 |
| 再現性 | 毎回同じ結果 | 時間やランダム性に依存 |
| 明確性 | 失敗時に原因が分かる | 失敗しても原因不明 |
| 焦点 | 1テスト1概念 | 複数の関心事が混在 |

### 命名

テスト名は期待される振る舞いを記述する。`should {期待する振る舞い} when {条件}` パターンを使う。

### 構造

- Arrange-Act-Assert パターン（Given-When-Then と同義）
- マジックナンバー・マジックストリングを避ける

## テスト戦略

- ロジックにはユニットテスト、境界にはインテグレーションテストを優先
- ユニットテストでカバーできるものにE2Eテストを使いすぎない
- 新しいロジックにE2Eテストしかない場合、ユニットテストの追加を提案する

## テスト環境の分離

テストインフラの設定はテストシナリオのパラメータに連動させる。ハードコードされた前提は別シナリオで壊れる。

| 原則 | 基準 |
|------|------|
| パラメータ連動 | テストの入力パラメータに応じてフィクスチャ・設定を生成する |
| 暗黙の前提排除 | 特定の環境（ユーザーの個人設定等）に依存しない |
| 整合性 | テスト設定内の関連する値は互いに矛盾しない |

```typescript
// ❌ ハードコードされた前提 — 別のバックエンドでテストすると不整合になる
writeConfig({ backend: 'postgres', connectionPool: 10 })

// ✅ パラメータに連動
const backend = process.env.TEST_BACKEND ?? 'postgres'
writeConfig({ backend, connectionPool: backend === 'sqlite' ? 1 : 10 })
```
