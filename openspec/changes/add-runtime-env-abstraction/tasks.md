## 1. 調査
- [ ] `ActorRuntimeMutex::new` および `SpinSyncMutex::new` を直接呼び出している箇所を洗い出し、`RuntimeEnv` へ置き換える対象をリストアップする
- [ ] `ActorSystemBuilder` / `ActorSystemConfig` の初期化フローを確認し、環境注入の導線を設計する

## 2. 実装
- [ ] `RuntimeEnv` トレイトと標準実装 (`NoStdEnv` / `StdEnv`) を追加する
- [ ] `ActorSystemBuilder` / `ActorSystemConfig` に環境設定 API を導入し、デフォルト環境を `NoStdEnv` に設定する
- [ ] ランタイム内部で `RuntimeEnv` を保持し、同期プリミティブ生成を環境経由にリファクタリングする

## 3. 検証・ドキュメント
- [ ] ユニットテストまたはサンプルで `NoStdEnv` / `StdEnv` が期待通り動作することを確認する
- [ ] `actor-std` から `StdEnv` を再エクスポートし、利用ガイド・サンプルを更新する
- [ ] `./scripts/ci-check.sh all` を実行し回帰がないことを確認する
