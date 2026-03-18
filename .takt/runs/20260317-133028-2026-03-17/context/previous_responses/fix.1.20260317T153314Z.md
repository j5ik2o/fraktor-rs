バックグラウンドタスクの失敗は、以前の調査用コマンド（`tokio-transport` feature なしでテスト一覧を取得しようとした）によるもので、修正作業には影響ありません。

全ての修正は既に完了しており、テスト結果も確認済みです：
- remote: 127 passed
- actor: 1114 passed
- CI check-unit-sleep: 違反なし