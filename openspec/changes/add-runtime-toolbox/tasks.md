## 1. 調査
- [ ] `ActorRuntimeMutex::new` および `SpinSyncMutex::new` を直接呼び出している箇所を洗い出し、`RuntimeToolbox` へ置き換える対象をリストアップする
- [ ] `ActorSystemBuilder` / `ActorSystemConfig` の初期化フローを確認し、環境注入の導線を設計する

## 2. 実装
- [ ] `RuntimeToolbox` トレイトと標準実装 (`NoStdToolbox` / `StdToolbox`) を追加する
- [ ] `ActorSystemBuilder` / `ActorSystemConfig` に環境設定 API を導入し、デフォルト環境を `NoStdToolbox` に設定する
- [ ] ランタイム内部で `RuntimeToolbox` を保持し、同期プリミティブ生成を環境経由にリファクタリングする

## 3. 検証・ドキュメント
- [ ] `NoStdToolbox` と `StdToolbox` の切替テストを追加する
- [ ] ドキュメントとサンプル（actor-std 向け）を更新し、環境設定手順を記載する
- [ ] 環境切替が実行後に行えないことを仕様／ガイドに明示する
- [ ] `./scripts/ci-check.sh all` を実行し回帰がないことを確認する
