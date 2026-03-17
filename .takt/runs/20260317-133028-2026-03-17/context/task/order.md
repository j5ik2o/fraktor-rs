# 時間依存テスト再設計プラン

更新日: 2026-03-17

## 概要

単体テストでは実時間依存を原則排除し、論理時間で振る舞いを検証する構成へ寄せる。
既存 repo では `ManualTestDriver` を使った決定的テスト基盤が既にあるため、それを標準パターンに引き上げる。
一方で、実ランタイム・実 transport・実スレッドの相互作用を見るテストは統合テストとして残し、
`ci-check` では unit / integration / long-running を分離する。

成功条件は次の通りとする。

- 単体テスト層に `thread::sleep` / `tokio::time::sleep` / 壁時計待ちを残さない
- 時間依存ロジックは fake/manual time で検証できる
- 実時間依存が必要なテストは統合テスト層へ移し、CI で実行経路を分ける
- `ci-check` の default 実行で長時間テストがボトルネックにならない

## 重要な変更方針

### 1. テスト階層を明文化する

- 単体テスト: 実時間待ち禁止。論理時間の前進、状態遷移、イベント、再試行回数、バックオフ列だけを検証する
- 統合テスト: 実ランタイム、実 transport、実 actor system の接続を確認する。実時間待ちは許可するが最小化する
- 長時間/耐久テスト: soak、multi-node、実ネットワークのような重いものは別枠に分離する

### 2. 時間を外部依存として扱う共通方針に揃える

- `actor` / `remote` / `cluster` / `streams` の時間依存ロジックは、可能な限り `ManualTestDriver`、tick 前進、手動 scheduler、fake clock に寄せる
- 既に manual tick を使っている箇所を共通ヘルパへ寄せ、各テストが `sleep` ではなく `advance` / `pump` / `run_scheduler` で進むようにする
- 壁時計が必要な箇所は `Clock` / `NowProvider` 相当の注入点を追加して fake 実装を渡せるようにする
- `actor` の circuit breaker 系は TODO がある通り clock 抽象を導入し、`thread::sleep` / `tokio::sleep` を fake clock 前進へ置き換える

### 3. 既存テストを 3 グループに分けて移行する

#### A. 単体へ寄せる対象

- `modules/actor/src/std/pattern/circuit_breaker*.rs` の sleep 依存
- `modules/actor/src/std/scheduler/tick/tests.rs` の実時間待ち
- `modules/actor/src/std/system/base/tests.rs` の短い待機
- fake/manual time に寄せ、状態変化を直接 assert する

#### B. 統合のまま残すが待ち方を改善する対象

- `modules/remote/src/std/endpoint_transport_bridge/tests.rs`
- `modules/remote/tests/quickstart.rs`
- `modules/remote/tests/multi_node_scenario_integration.rs`
- 固定 sleep を「状態が変わるまで待つ」ヘルパ、イベント受信、channel/barrier、手動 tick 前進へ置き換える

#### C. 実時間統合として明示的に残す対象

- `modules/remote/src/std/transport/tokio_tcp/tests.rs`
- `modules/cluster/src/std/tokio_gossip_transport/tests.rs`
- 実 transport / 実 runtime 契約確認のため残す。ただし待機時間は短縮し、待ち理由をコメントで固定する

### 4. CI をテスト階層に合わせて分離する

- `scripts/ci-check.sh` に fast unit と integration を分けたサブコマンドを追加する
- `all` は原則 full だが、AI 実行や日常開発で使う経路は fast unit を先に通す構成にする
- `workspace test --examples --tests` を一括で長時間回す経路は維持してもよいが、default 開発導線からは外す
- `HANG_SUSPECT` は「hang」より「長時間統合テスト超過」を示すことが多いため、guard 対象を unit と integration で別 timeout に分ける
  - unit: 短い timeout
  - integration/full: より長い timeout、または crate 単位分割

### 5. ポリシー違反を自動検出する

- unit テスト対象パスでは `sleep` / 実時間 `timeout` を禁止する軽量チェックを追加する
- allowlist 方式で統合テスト対象だけ例外にする
- 最低限、`ci-check` で grep ベースの検査を追加し、unit パスに新しい実時間待ちが入ったら fail させる

## 実装変更の詳細

### actor

- `circuit_breaker` / `circuit_breaker_shared` に fake clock 注入点を追加する
- scheduler/tick の std テストは tokio sleep ではなく manual tick 前進で検証する
- `recv_timeout` を使う排他/並行性テストは「時間経過の検証」ではなく「同期の失敗検知」なので unit のまま残す

### remote

- `endpoint_transport_bridge` テストの固定 sleep を段階的に撤去する
- 既に `bridge.now_millis()` と manual tick driver があるので、handshake timeout 系は manual clock / tick で進める方向を第一候補にする
- transport open/send 遅延検証は channel 通知や condition wait に置き換え、固定 80ms/200ms 待ちを削減する
- `quickstart` / `multi_node` は統合テストとして明示し、unit と混ぜない

### cluster

- `tokio_gossip_transport` の実時間待ちは統合テスト扱いに固定する
- membership/gossip のロジック自体は `TimerInstant` / manual tick を使う unit テストを優先し、実時間依存を広げない

### streams

- `ActorMaterializer` や timer graph 系は既に manual tick で寄せられるので、今後の標準パターンとして横展開する
- streams 側には新たな sleep 禁止ポリシーだけ適用し、今回の主対象は remote/actor に置く

## テスト計画

- 単体テスト層から `thread::sleep` / `tokio::time::sleep` を除去したことを自動検査で確認
- `actor` の circuit breaker 系は fake clock 前進のみで同じ期待値を維持することを確認
- `remote` の handshake / quarantine / flush / heartbeat 系は、sleep 削除後も既存の状態遷移 assertion が維持されることを確認
- `ci-check` の fast unit 経路が従来より短時間で終わることを確認
- full integration 経路が従来どおり全パスすることを確認
- `HANG_SUSPECT` が unit 経路では再発しないことを確認
- 統合テスト allowlist 外に新しい実時間待ちが追加された場合に CI が失敗することを確認

## 前提

- 今回の成果物は「repo 全体の時間依存テストの整理計画」と「それを実行するための decision-complete な方針」であり、実装は次フェーズで行う
- 単体テストでは実時間待ちを原則禁止する
- 実 transport / 実 runtime 相互作用を確認するテストは統合テストとして残す
- `ManualTestDriver` / tick 前進 / fake clock を repo 標準の時間制御手段として採用する
- backward compatibility は不要なので、テスト API や `ci-check` サブコマンドは必要に応じて破壊的に整理してよい