1. [x] Behavior trait / 遷移結果型 / same・stopped・ignore の基盤実装を追加し、ドキュメントを整備する
2. [x] Behaviors ビルダーで receiveMessage / receiveSignal を実装し、返却 Behavior に遷移するロジックを組み込む
3. [x] Behavior を TypedProps から起動できるアダプタを実装し、テストダブルでガーディアン起動を確認する
4. [x] 代表的な遷移（same, stopped, ignore, 別 Behavior への切り替え）をカバーする単体テストを追加する
5. [x] 新 API を使う最小 example を `examples/` に追加し、CI ドキュメントへ実行手順を追記する
6. [x] `./scripts/ci-check.sh all` を実行し、静的解析・テストをすべてパスする
