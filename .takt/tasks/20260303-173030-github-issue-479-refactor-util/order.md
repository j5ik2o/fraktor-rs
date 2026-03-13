## GitHub Issue #479: refactor: utils の pub 可視性整理（内部型の pub 剥がし）

Parent issue: #410

## 目的

`modules/utils/` で他モジュールから参照されていないが utils 内部で使用されている型の `pub` を `pub(crate)` に縮小し、公開 API を最小化する。

## 背景

Sub 1 (#477)、Sub 2 (#478) で未使用型を削除した後、残る型のうち約70型が utils 内部でのみ使用されている。これらの `pub` を剥がすことで公開型数を大幅に削減できる。

主な対象領域:
- `queue/` 内部型 (~38型): `AsyncQueueBackend`, `SyncQueueBackend`, `MpscKey`, `SpscKey`, `PriorityKey`, `TypeKey`, 各種 Producer/Consumer Shared 型等
- `sync/` 内部型 (~14型): `SendBound`, `SharedBound`, `SharedDyn`, `SharedError`, `SharedFactory`, `Flag`, `InterruptContextPolicy` 等
- `std/` 内部型 (~8型): `MpscBackend`, `StdSyncFifoQueueShared`, `StdSyncMutexGuard` 等

## タスク

- [ ] Sub 1, Sub 2 完了後に着手
- [ ] 他モジュール未参照 & utils 内部でのみ使用の型を特定
- [ ] `pub` → `pub(crate)` への変更
- [ ] `mod.rs` のリエクスポート整理
- [ ] `./scripts/ci-check.sh ai all` でグリーン確認

## 受け入れ基準

- 他モジュールから参照される型のみが `pub` であること
- CI が全パスすること
- 他モジュールのコンパイル・テストに影響がないこと

## 推定変更ファイル数

~40ファイル

## 依存関係

#477, #478 の完了後に着手

### Labels
refactoring
