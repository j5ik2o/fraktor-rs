## GitHub Issue #477: refactor: utils の stack/ と concurrent/ 全削除

Parent issue: #410

## 目的

`modules/utils/` から他モジュール未参照の `collections/stack/` と `concurrent/` サブモジュールを全削除し、不要コードを除去する。

## 背景

- `stack/` 全13型: `AsyncStack`, `SyncStack`, `VecStackBackend`, `StackError` 等 — 他モジュールから参照ゼロ
- `concurrent/` 全11型: `AsyncBarrier`, `CountDownLatch`, `WaitGroup`, `Synchronized` 等 — 他モジュールから参照ゼロ

## タスク

- [ ] `modules/utils/src/core/collections/stack/` ディレクトリを削除
- [ ] `modules/utils/src/core/concurrent/` ディレクトリを削除
- [ ] 関連する `mod.rs` のリエクスポート・モジュール宣言を除去
- [ ] 関連テストファイルを削除
- [ ] `./scripts/ci-check.sh all` でグリーン確認

## 受け入れ基準

- stack, concurrent 関連の型が `pub` エクスポートから消えていること
- CI が全パスすること
- 他モジュールのコンパイル・テストに影響がないこと

## 推定変更ファイル数

~35ファイル

### Labels
refactoring