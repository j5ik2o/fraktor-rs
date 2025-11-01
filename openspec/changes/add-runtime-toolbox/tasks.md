## 1. 調査
- [ ] `ActorRuntimeMutex::new` と `SpinSyncMutex::new` を直接呼び出している箇所を洗い出し、`SyncMutexFamily::create` へ置換すべき範囲を整理する
- [ ] `ActorSystem` / `ActorSystemState` / `Mailbox` / `Dispatcher` など、ミューテックスを保持する内部構造の依存関係を確認する

## 2. 実装
- [ ] `SyncMutexFamily` / `RuntimeToolbox` / `NoStdToolbox` / `StdToolbox` を実装し、ファミリー単体テストを追加する
- [ ] ランタイム内部で `ToolboxMutex<T, TB>` のような型エイリアスを導入し、`SyncMutexFamily::create` を用いるようリファクタリングする
- [ ] `ActorSystemBuilder` などにツールボックス選択 API（例: `with_toolbox::<StdToolbox>()`）と `StdActorSystem` エイリアスを追加し、公開 API は既存シグネチャを維持する

## 3. 検証・ドキュメント
- [ ] `StdToolbox` 選択時の統合テスト（例: 既存の std 対応サンプル）を追加または更新し、エイリアス経由でも従来通り動作することを確認する
- [ ] ドキュメントとサンプル（特に actor-std）を更新し、ビルダー API / エイリアスを用いた環境切り替え手順を記載する
- [ ] ランタイム起動後にツールボックスを変更できない旨と、既定が `NoStdToolbox` であることを明示する
- [ ] `./scripts/ci-check.sh all` を実行し回帰がないことを確認する
