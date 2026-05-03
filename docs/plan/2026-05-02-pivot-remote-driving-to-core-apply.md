# pivot-remote-driving-to-core 実装計画

対象 change: `openspec/changes/pivot-remote-driving-to-core/`

## 前提

- OpenSpec CLI 互換スクリプトでは `.openspec.yaml` が無いため active change として認識されないが、`proposal.md` / `design.md` / `tasks.md` は存在する。
- 実装は `tasks.md` の依存順に進める。
- 新規公開型は `RemoteEvent` と `RemoteEventReceiver` の 2 つに抑える。
- `Remote` はジェネリクス化せず、instrument は `Box<dyn RemoteInstrument + Send>` で保持する。

## 実装順序

1. core 側 instrument 配線基盤を追加する。
2. Association に instrument hook、handshake generation、watermark query を追加する。
3. core 側に `RemoteEvent` / `RemoteEventReceiver` / handshake timeout scheduling port を追加する。
4. `Remote::run` を event loop として実装し、effect 実行経路を core に集約する。
5. `RemoteConfig` に outbound watermark 設定を追加する。
6. std adapter を event push 型に縮退し、`Remote::run` spawn 経路へ切り替える。
7. 旧 outbound loop / handshake driver を削除し、残る dead code を整理する。
8. 制約 grep、package 単位テスト、最終 CI を実行する。

## 検証

- 途中では対象 crate の test / check に留める。
- ソースコード編集後の最後に `./scripts/ci-check.sh ai all` を実行する。
