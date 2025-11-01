## 1. 調査
- [ ] `ActorRuntimeMutex::new` を直接呼び出している箇所を洗い出し、`TB::SyncMutex<T>::new` への置換対象を整理する
- [ ] `ActorSystem` / `ActorSystemState` / `ActorSystemBuilder` が保持する型情報を確認し、ジェネリクス化の影響範囲を把握する

## 2. 実装
- [ ] `RuntimeToolbox` を GAT ベースの関連型マッピングとして実装し、`NoStdToolbox` / `StdToolbox` を整備する
- [ ] `ActorSystemGeneric<TB>` / `ActorSystemBuilder<TB>` / `ActorRuntimeMutex<TB>` など必要な構造体・型エイリアスを導入する
- [ ] ランタイム内部のデータ構造を `TB::SyncMutex<T>::new` 形式にリファクタリングする

## 3. 検証・ドキュメント
- [ ] `ActorSystemGeneric<NoStdToolbox>` / `<StdToolbox>` のユニットテストを追加し、推論が崩れないことを確認する
- [ ] ドキュメントとサンプル（特に actor-std）を更新し、型パラメータで環境を選択する手順を記載する
- [ ] ランタイム起動後に環境を変更できないこと、および既定が `NoStdToolbox` であることを明示する
- [ ] `./scripts/ci-check.sh all` を実行し回帰がないことを確認する
