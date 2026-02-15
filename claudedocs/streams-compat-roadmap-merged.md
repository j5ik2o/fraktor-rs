# fraktor-rs streams 互換化ロードマップ（統合版）

最終更新日: 2026-02-14

このドキュメントは次の3件を統合したものです。

- `claudedocs/pekko-streams-compat-plan.md`
- `claudedocs/pekko-streams-missing-operators.md`
- `claudedocs/streams-failure-model.md`

## 1. 目的

現行 `modules/streams` を、Pekko 互換を重視しつつ段階的に引き上げる。  
過剰な機能追加は避け、必要最小限の API とセマンティクスのみを追加する。

- 仕様優先度は互換性: `Pekko互換がMUST`、独自拡張は原則追加しない
- 後方互換性は保持しない（開発フェーズとして最適設計を優先）
- 変更は意味のある単位で分割し、各段階でテストと lint を通す
- 実行は `./scripts/ci-check.sh all` を最終合格条件とする

## 2. 現状サマリ

`modules/streams` は現時点で以下の特徴を持つ。

- 実行モデルは1アクター（1 island）寄りであり、同一実行系の中心化が強い
- `FlowAsyncBoundary` は現在「バッファ付きの経路分離」に近く、真の非同期境界（別 actor / 別タスク）ではない
- 既存の `Source -> Flow* -> Sink` は主に線形 DSL 前提
- 既に `Shape` 化や graph DSL、再起動/監督連携は一部導入されているが、内部全体としては不完全な接続が残る

## 3. 達成条件

1. 線形専用の制約を外し、Graph/Junction/Substream を実用的に扱える
2. 障害回復（recover, supervision, restart）を実行器に接続し、no-op を排除する
3. 非同期境界分離（実行分離）を実現できる形に寄せる
4. 回帰テストを固定し、`./scripts/ci-check.sh all` を通す

## 4. 参考（互換観点）

- Pekko の資料: `references/pekko/docs/src/main/paradox/stream`
- 機能差分元: `references/pekko/docs/src/main/paradox/stream/operators/index.md`
- 実装対象: `modules/streams/src/core`

## 5. 残タスクの全体像

### 5.1 既知の実装済み

- [x] `balance`
- [x] `broadcast`
- [x] `buffer`
- [x] `concat`
- [x] `drop`
- [x] `dropwhile`
- [x] `empty`
- [x] `filter`
- [x] `filternot`
- [x] `flattenoptional`
- [x] `flatmapconcat`
- [x] `flatmapmerge`
- [x] `fold`
- [x] `fromarray`
- [x] `fromiterator`
- [x] `fromoption`
- [x] `foreach`
- [x] `groupby`
- [x] `grouped`
- [x] `head`
- [x] `ignore`
- [x] `intersperse`
- [x] `last`
- [x] `map`
- [x] `mapconcat`
- [x] `mapoption`
- [x] `merge`
- [x] `recover`
- [x] `recoverwithretries`
- [x] `scan`
- [x] `single`
- [x] `sliding`
- [x] `splitafter`
- [x] `splitwhen`
- [x] `statefulmap`
- [x] `statefulmapconcat`
- [x] `take`
- [x] `takeuntil`
- [x] `takewhile`
- [x] `zip`
- [x] `zipwithindex`

### 5.2 未実装（カテゴリ別）

Pekko側アンカー総数: 210、実装済み推定: 41、未対応: 169。

Source operators

- [ ] `assourcewithcontext`
- [ ] `assubscriber`
- [ ] `combine`
- [ ] `completionstage`
- [ ] `completionstagesource`
- [x] `cycle`
- [x] `failed`
- [x] `from`
- [ ] `fromjavastream`
- [ ] `frompublisher`
- [ ] `future`
- [ ] `futuresource`
- [x] `iterate`
- [ ] `lazycompletionstage`
- [ ] `lazycompletionstagesource`
- [ ] `lazyfuture`
- [ ] `lazyfuturesource`
- [ ] `lazysingle`
- [ ] `lazysource`
- [ ] `maybe`
- [x] `never`
- [ ] `queue`
- [x] `range`
- [x] `repeat`
- [ ] `tick`
- [ ] `unfold`
- [ ] `unfoldasync`
- [ ] `unfoldresource`
- [ ] `unfoldresourceasync`
- [ ] `zipn`
- [ ] `zipwithn`

Sink operators

- [ ] `aspublisher`
- [ ] `cancelled`
- [ ] `collect`
- [ ] `collection`
- [ ] `completionstagesink`
- [ ] `count`
- [ ] `exists`
- [ ] `foldwhile`
- [ ] `forall`
- [ ] `foreachasync`
- [ ] `frommaterializer`
- [ ] `fromsubscriber`
- [ ] `futuresink`
- [ ] `headoption`
- [ ] `lastoption`
- [ ] `lazycompletionstagesink`
- [ ] `lazyfuturesink`
- [ ] `lazysink`
- [ ] `none`
- [ ] `oncomplete`
- [ ] `prematerialize`
- [ ] `reduce`
- [ ] `seq`
- [ ] `source`
- [ ] `takelast`

Additional sink and source converters

- [ ] `asinputstream`
- [ ] `asjavastream`
- [ ] `asoutputstream`
- [ ] `frominputstream`
- [ ] `fromoutputstream`
- [ ] `javacollector`
- [ ] `javacollectorparallelunordered`

File IO

- [ ] `frompath`
- [ ] `topath`

Simple operators

- [ ] `asflowwithcontext`
- [ ] `collectfirst`
- [ ] `collecttype`
- [ ] `collectwhile`
- [ ] `completionstageflow`
- [ ] `contramap`
- [ ] `detach`
- [ ] `dimap`
- [ ] `dooncancel`
- [ ] `doonfirst`
- [ ] `droprepeated`
- [ ] `foldasync`
- [ ] `futureflow`
- [ ] `groupedadjacentby`
- [ ] `groupedadjacentbyweighted`
- [ ] `groupedweighted`
- [ ] `lazycompletionstageflow`
- [ ] `lazyflow`
- [ ] `lazyfutureflow`
- [ ] `limit`
- [ ] `limitweighted`
- [ ] `log`
- [ ] `logwithmarker`
- [ ] `mapwithresource`
- [ ] `materializeintosource`
- [ ] `optionalvia`
- [ ] `scanasync`
- [ ] `throttle`

Flow operators composed of Sinks and Sources

- [ ] `fromsinkandsource`
- [ ] `fromsinkandsourcecoupled`

Asynchronous operators

- [ ] `mapasync`
- [ ] `mapasyncpartitioned`
- [ ] `mapasyncpartitionedunordered`
- [ ] `mapasyncunordered`

Timer driven operators

- [ ] `delay`
- [ ] `delaywith`
- [ ] `dropwithin`
- [ ] `groupedweightedwithin`
- [ ] `groupedwithin`
- [ ] `initialdelay`
- [ ] `takewithin`

Backpressure aware

- [ ] `aggregatewithboundary`
- [ ] `batch`
- [ ] `batchweighted`
- [ ] `conflate`
- [ ] `conflatewithseed`
- [ ] `expand`
- [ ] `extrapolate`

Nesting and flattening

- [ ] `flatmapprefix`
- [ ] `flattenmerge`
- [ ] `prefixandtail`
- [ ] `switchmap`

Time aware

- [ ] `backpressuretimeout`
- [ ] `completiontimeout`
- [ ] `idletimeout`
- [ ] `initialtimeout`
- [ ] `keepalive`

Fan-in operators

- [ ] `mergesequence`
- [ ] `concatalllazy`
- [ ] `concatlazy`
- [ ] `interleave`
- [ ] `interleaveall`
- [ ] `mergeall`
- [ ] `mergelatest`
- [ ] `mergepreferred`
- [ ] `mergeprioritized`
- [ ] `mergeprioritizedn`
- [ ] `mergesorted`
- [ ] `orelse`
- [ ] `prepend`
- [ ] `prependlazy`
- [ ] `zipall`
- [ ] `ziplatest`
- [ ] `ziplatestwith`
- [ ] `zipwith`

Fan-out operators

- [ ] `partition`
- [ ] `unzip`
- [ ] `unzipwith`
- [ ] `alsoto`
- [ ] `alsotoall`
- [ ] `divertto`
- [ ] `wiretap`

Watching status operators

- [ ] `monitor`
- [ ] `watchtermination`

Actor interop operators

- [ ] `actorref`
- [ ] `actorrefwithbackpressure`
- [ ] `ask`
- [ ] `askwithcontext`
- [ ] `askwithstatus`
- [ ] `askwithstatusandcontext`
- [ ] `sink`
- [ ] `watch`

Compression

- [ ] `deflate`
- [ ] `gzip`
- [ ] `gzipdecompress`
- [ ] `inflate`

Error handling

- [ ] `maperror`
- [ ] `onerrorcomplete`
- [ ] `onerrorcontinue`
- [ ] `onerrorresume`
- [ ] `onfailureswithbackoff`
- [ ] `recoverwith`
- [ ] `withbackoff`
- [ ] `withbackoffandcontext`

## 6. 障害モデル統一方針

以下を実装の共通ルールとする。

- Failure は制御失敗。`StreamError` として処理を停止させる
- Error は要素側エラー。要素型を `Result<T, StreamError>` で持つ
- `recover`/`recover_with_retries` は要素エラーペイロードを対象に復元する

現在の受理と現時点の状態

- `supervision_stop/resume/restart` は Source/Flow/Sink 側で受理され、GraphInterpreter 側の失敗分岐に接続される
- `restart_source_with_backoff`, `restart_flow_with_backoff`, `restart_sink_with_backoff` は graph へ反映され、tick ベース再起動の試行経路を持つ

再設計ルール

- no-op 実装を禁止し、実行器に supervisor と backoff を接続する
- restart は再起動対象演算子の状態初期化と継続可否を明文化
- kill switch と終了系（shutdown/abort）の状態遷移は実行 loop 側で一貫して扱う

## 7. async境界と並列化の現実的な立ち上げ方（ユーザー提起反映）

現在の非同期境界は「1 island 中の順次実行」を前提とした `FlowAsyncBoundary` であるため、まず以下を行う。

1. `async_boundary` を「非同期実体（別タスク/別 actor）に移す境界」の意味として再定義する
2. 実装を最小で導入し、まず `map_async` 系（mapasync 系）に限定して効果を検証する
3. その後 `buffer` + `FlowAsyncBoundary` + `restart/supervision` との整合を固定し、順序性/バックプレッシャー/キャンセル伝播を確立する

## 8. フェーズ化ロードマップ（短期 / 中期 / 長期）

短期

1. no-op 解消: supervision と restart を実行器に接続
2. async boundary の実体化に向けた最小実装（mapasync 系を含む）
3. timer/backpressure 非同期系の高頻度利用演算子を上位優先で実装
- `delay`/`initialdelay`/`takewithin`
- `batch`/`conflate`/`expand`
- `throttle`
4. `Zip`/`Merge`/`Broadcast`/`Balance` の既存挙動を並列境界前後でも回帰確認

中期

1. fan-in/fan-out の未実装を増補
- `partition`/`mergeall`/`zipall`/`zipwith`
- `concatalllazy`/`prepend`/`prependlazy`/`interleave`/`interleaveall`
2. substream と group/flat 系演算子の組み合わせ検証
3. テストで再現しにくい順序・完了・失敗条件を固定し、`merge_substreams` 系を含む回帰を増やす

長期

1. Java/Pub/Sub/Future 相当の外部インテグレーション系の導入
- `completionstage` 系、publisher 系、ファイル I/O 系
2. actor 連携・kill/hub 的機能の実用安定化
3. 残る圧縮/高度エラー系 (`withbackoff`, `onerror*`, `gzip*`) を段階導入
4. 追加オペレーター総数の「未対応数推移」を毎フェーズ更新し、残数の実態を明示

## 9. テスト要件（フェーズ横断）

1. 線形パイプの既存回帰
2. Junction 系（Broadcast/Merge/Zip/Concat）
3. substream 系（group_by / split / merge_substreams）
4. recovery 系境界（fallback, retries, backoff）
5. kill switch の `shutdown` / `abort` 影響確認
6. Demand/完了/エラーの厳密検証（必要なら TestSource/TestSink probe）
7. `no_std` 互換部分の維持検証
8. 各フェーズ完了時に対象範囲テスト、最終に `./scripts/ci-check.sh all`

## 10. Phase 1〜4（統合実行計画）

### Phase 1: グラフ基盤互換化（最優先）

- `Shape` 抽象導入と既存 `StreamShape` からの移行
- `StreamGraph` をステージ列からノード+ポート+エッジへ
- `StreamPlan` を実行器向けグラフ中間表現へ
- 実行器をポート駆動へ再設計し、複数in/outとjunctionを扱えるように
- `Graph` 不変blueprint と materialization 境界を明文化
- 線形 API は互換アダプタとして維持

### Phase 2: 中核互換演算子

- fan-out / fan-in の主要演算子追加
- flat_map 系と buffer/async boundary の実質的再定義
- substream (`group_by` 系, `merge/concat_substreams`)
- GraphDSL 的な部分グラフ構築 API

### Phase 3: 障害モデル統合（今回最重要）

- supervision `Stop/Resume/Restart` 実効化
- `recover` と `recover_with_retries` の厳密化
- `Restart*` with backoff を実行器/時間計測と接続
- failure/error 取り扱いをテストで固定

### Phase 4: 動的制約と検証基盤

- KillSwitch 系、Hub 系、TestKit 系 probe の完成
- fuzz 相当のストレスシナリオを導入し、順序/停止系/再起動系を増刷る
- 最終受け入れ基準を満たすまで反復

## 11. リスクと回避

- 実行器刷新による既存破綻: フェーズ順守と回帰固定で緩和
- backoff 再起動の複雑化: failure model を先に固める
- API 膨張: Pekko互換MUSTのみ採用し、不要機能は延期
- 未完成状態の no-op 機能が残ること: no-op の存在自体を禁止基準に入れる

## 12. 成果物

- この1ファイルを streams の一次計画として採用する
- 旧3ファイルは削除し、今後はこのファイルのみを更新対象とする

## 13. タスクリスト（実装順）

### 13.1 P0: まず着手する前提タスク

1. [x] P0-01 目標整合: この md を唯一の streams 計画原本として固定する
2. [x] P0-02 現行テストの実行範囲（回帰）を確定し、壊れやすいテストを識別する
3. [x] P0-03 `modules/streams` の `async_boundary` と `GraphInterpreter` の現状差分を最小再現テストで固定する
4. [x] P0-04 no-op 監視・再起動 API の現状を再確認し、対象 API 一覧を確定する

#### 13.1.x P0 実行ログ

- 実行コマンド:
  - `cargo test -p fraktor-streams-rs async_boundary`
  - `cargo test -p fraktor-streams-rs restart`
  - `cargo test -p fraktor-streams-rs supervision`
- 結果: いずれも `ok`。対象は 7/17/5 件（フィルタ一致分）を確認
- no-op 監視・再起動 API（設定反映先を確認）:
  - `Source::{restart_source_with_backoff, restart_source_with_settings, supervision_stop, supervision_resume, supervision_restart}`
  - `Flow::{restart_flow_with_backoff, restart_flow_with_settings, supervision_stop, supervision_resume, supervision_restart}`
  - `Sink::{restart_sink_with_backoff, restart_sink_with_settings, supervision_stop, supervision_resume, supervision_restart}`
- 検証結果の要約:
  - 設定自体は graph に反映され、GraphInterpreter 側で restart / supervision ハンドリング経路に到達することを確認
  - ただし、複合経路・非同期境界混在時の完全集約性は P3 で追加検証する

### 13.2 P1: グラフ基盤固定（優先度: 最高）

1. [x] P1-01 `Shape` 系（`Shape`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`）の公開 API を定義し直す
2. [x] P1-02 `StreamGraph` をステージ列から「ノード+ポート+エッジ」に再設計する
3. [x] P1-03 `StreamPlan` を graph 実行向け中間表現へ置換する
4. [x] P1-04 `StreamInterpreter`/`GraphInterpreter` をポート駆動化し、複数in/out を扱えるように変更する
5. [x] P1-05 線形 `StreamShape` API の互換アダプタを維持しつつ新基盤へ接続する
6. [x] P1-06 junction/fan-in/fan-out を破綻しない形で最小実行経路として通せるようにする
7. [x] P1-07 Graph の materialization 境界と不変性契約をドキュメントとテストで固定する
8. [x] P1-08 P1 対象テスト（回帰）を追加して `ci-check` 対象を更新する

#### 13.2.x P1 現状スナップショット

- `StreamGraph` はすでに `nodes/edges/ports` を持つ構成になっており、`into_plan` は `source_count == 1` / `sink_count == 1` の線形制約を撤廃している
- `Shape` 系は `shape/bidi_shape.rs` / `flow_shape.rs` / `source_shape.rs` / `sink_shape.rs` が存在し、基盤側の型は概ね揃っている
- `GraphInterpreter` 側は `graph_interpreter.rs` の `compile_plan` で source/sink の 1 対 1 制約を解消し、複数in/out を扱える状態に変更している
- `StreamPlan::from_parts` で fan-in / fan-out / cycle など実行器向け変換の主要不正を検証し、`P1-03` は完了
- P1 回帰として、`Shape` API と materialization 境界の回帰テストを追加済み
  - `converts_between_stream_shape_and_flow_shape`
  - `converts_source_shape_to_stream_shape`
  - `converts_sink_shape_to_stream_shape`
  - `with_shared_kill_switch_keeps_materialized_value`
- 次アクション: `P2-01` 完了（`broadcast`/`balance`/`merge`/`zip`/`concat` を新基盤へ接続）
### 13.3 P2: オペレーター拡張（実用性コア）

1. [x] P2-01 `broadcast`/`balance`/`merge`/`zip`/`concat` の既存実装を新 Graph 基盤へ接続する
2. [x] P2-02 `map_async` 系を実装し、実効する async 境界の最小検証を通す
3. [x] P2-03 `flat_map_merge` と `flat_map_concat` の順序・並行・終了条件を固定テスト化する
4. [x] P2-04 `buffer`, `throttle`, `batch` 系を backlog 抑制含めて追加実装する
5. [x] P2-05 `group_by`, `split_when`, `split_after`, `merge_substreams`, `concat_substreams` を実装する
6. [x] P2-06 `GraphDSL` の最小 partial graph API を追加し、`from_*` 系で利用可能にする
7. [x] P2-07 `BidiFlow` の最小骨格を追加する
8. [x] P2-08 `delay`, `initialdelay`, `takewithin` 等 timer 系を追加する
9. [x] P2-09 `partition`/`unzip` 系を段階追加する
10. [x] P2-10 `Fan-in` 主要演算子 (`interleave`, `prepend`, `zipall`) を追加する
11. [x] P2-11 P2 対象テスト（junction/substream/backpressure）を追加する

#### 13.3.x P2 現状スナップショット

- 方針
  - 今フェーズでは後方互換維持を優先せず、再設計に不要な互換分岐は持ち込まない
  - `P2` で新規追加する挙動は既存実装を流用できる場合は共通化し、同一ロジックの重複は `DRY` で抑える
- 進捗
  - `map_async`, `flat_map_merge`, `flat_map_concat`, `group_by`, `split_when`, `split_after`, `buffer`, `throttle`, `batch`, `merge_substreams`, `concat_substreams` の主要経路は既に実装・テストが存在
  - `GraphDSL` の最小構成と `BidiFlow` は既存骨格があり、`P2-06` / `P2-07` の到達基準を満たす
- 未完了（優先順）
  - `P3`: 障害モデル（no-op 解消）
  - `P4`: 動的制御・検証基盤

#### 13.3.y P2 続行アクション

- まず着手する順
  1. `timer` 系ステート管理を共通化して、`delay`/`initialdelay`/`takewithin` 相当の構造を先に固定
  2. `partition` と `unzip` を `Flow`/`Source` の双方で使い回し可能な定義に寄せる
  3. `interleave` / `prepend` / `zipall` を `Fan-in` の既存分岐（`merge` 系、`zip` 系、`concat` 系）で共通化しつつ実装
  4. `P2-11` を `supervision/backpressure/join/segment` の観点で固定し、`Phase3` へ移すために最小の回帰セットを確定
- 完了条件
  - `P2-08`/`P2-09`/`P2-10` のチェックを `[x]` へ更新
  - `P2-11` の追加テストを `./scripts/ci-check.sh all` の対象に反映しないまでも、`P2` 範囲の再現失敗が再発しない状態を固定
  - 互換分岐を増やさず、既存実装は可能な限り流用で DRY を維持

#### 13.3.z P2-08 実施計画（timer 系）

- 到達目標
  - `delay`, `initialdelay`, `takewithin` を timer 系として同一の時刻計測抽象で扱う
  - `Flow` と `Source` で差分定義を最小化し、実装ロジックは可能な限り共通化
- タスク分解
  - [x] P2-08-01 `timer` 用の中核定義を設計し、`GraphInterpreter` での時間制御ノードと接続仕様を固定する
  - [x] P2-08-02 `delay` を `Flow`/`Source` 向けに実装し、要素順序と完了の時系列を固定テスト化する
  - [x] P2-08-03 `initialdelay` を実装し、空入力/即完了時の振る舞いを境界条件として追加する
  - [x] P2-08-04 `takewithin` を実装し、タイムアウト境界での途中完了・要素残留ルールを固定する
  - [x] P2-08-05 timer 系テストを 1 まとまりに整理し、`P2-11` の回帰観点（junction/substream/backpressure）へ接続する
- 受け入れ条件
  - DRY: `delay` と `initialdelay` は同一の timer 基底に寄せる
  - no-op 禁止: `P2-08` の各項目は既存 no-op 実装を残さない
  - 形式: `P2-08-05` 完了時点で `P2-08` を `[x]` 更新可能にする

#### 13.3.z.x P2-08-01 timer 基底具体化

- 現時点判断
  - ここを先に固定しないと `P2-08-02`〜`P2-08-04` の実装が分散するため、先行着手対象は `P2-08-01` に限定
- スコープ
  - 時刻管理（tick 系設定値）を持つ共通ヘルパーを `Graph` 定義層で導入
  - timer 演算子共通の制御状態を 1 箇所に抽象化し、`Flow`/`Source` 側 API は薄いエントリポイントに集約
  - 完了・キャンセル・エラー処理は共通ヘルパーのみで完結
- 実装分解（順序固定）
  - `timer` 名義の時刻抽象を `StreamPlan` 側のノード属性として定義し、`GraphInterpreter` での参照先を明確化
  - `duration`/`deadline`/`deadline_at_open` の表現を共通化し、`initialdelay` と `delay` の差分はオプション定義として吸収
  - `takewithin` 追加時の「未完了要素保持/破棄」の境界を、既存 backpressure 終端ハンドリングと一体で検証
- 完了条件（P2-08-01）
  - `P2-08-01` 実装内容を 1 つの定義エントリとして `default_operator_catalog` 側に反映できる状態
  - `P2-08-01` の観点で `map_async` 系の実効検証に必要なタイム依存ロジックが再利用可能
  - `P2-08` 以外のカテゴリに timer 名の複製実装を持たない

#### 13.3.w P2-09 実施計画（分岐・逆配列系）

- 到達目標
  - `partition` と `unzip`/`unzip_with` を、既存 `Graph`/`Shape` 経路のまま実効化する
  - `Flow` と `Source` のサブクラス別差分は最小化し、実装共通化を優先する
- タスク分解
  - [x] P2-09-01 `partition` の `Graph` 定義を整備し、`Fan-out` の順序・分配規則を固定する
  - [x] P2-09-02 `unzip` と `unzip_with` の共通定義を追加し、`flow`/`source` 側へ露出する
  - [x] P2-09-03 `partition`/`unzip` 系の backpressure/完了/エラー挙動を固定テスト化する
  - [x] P2-09-04 `group_by` や `split/merge_substreams` と組み合わせた組合せ回帰を 1 セット追加する
- 受け入れ条件
  - DRY: `partition`/`unzip` は `StreamShape` 方向の重複ロジックを避け、定義レベルで分離する
  - 実装単位: `P2-09-04` 完了時点で `P2-09` を `[x]` 更新可能にする
  - 監査: `P2` 全体の進捗計画（`P2-11`）へ遷移可能な回帰観点を 1 つ以上追加する

#### 13.3.v P2-10 実施計画（主要 fan-in）

- 到達目標
  - `interleave`、`prepend`、`zipall` を既存の junction 実行系に載せ、`merge`/`concat`/`zip` 系との一貫性を保つ
  - `merge_preferred` / `prepend` 系と内部的に重なりうる処理は 1 箇所に集約する
- タスク分解
  - [x] P2-10-01 `interleave` の入力分岐ポリシーと配信順序を明文化し、`Flow` 実装を追加
  - [x] P2-10-02 `prepend` / `prepend_lazy` の差分を共通化し、`append` 系とは別に最小 API で提供
  - [x] P2-10-03 `zipall` の終了条件・不足要素ハンドリングを固定し実装追加
  - [x] P2-10-04 `P2-10` 追加演算子の junction 側回帰を `P2-11` 観点へ接続する
- 受け入れ条件
  - DRY: fan-in 系で重複する要素の配布・drain ロジックは共通ヘルパーへ統合
  - 整合性: 既存 `interleave`/`prepend` 系 API とテストでシグネチャ重複を排除
  - 完了: `P2-10-04` にて `P2-10` を `[x]` 更新可能にする

#### 13.3.u P2-11 方針（回帰固定）

- 方針
  - `P2-08`/`P2-09`/`P2-10` の実装は、同一カテゴリ内で共通に壊れやすい境界条件を固定する
  - no-op 実装を増やさず、`GraphInterpreter` 側の制御境界（需要/完了/失敗）で検証可能な形にする
- 対応項目
  - [x] P2-11-01 `delay/initialdelay/takewithin` の時間境界と backpressure テストを追加
  - [x] P2-11-02 `partition/unzip` の order/backpressure/完了連鎖を固定
  - [x] P2-11-03 `interleave/prepend/zipall` の fan-in 収束と終端状態を固定
  - [x] P2-11-04 `group_by`, `merge_substreams`, `concat_substreams` と新規 `P2-08`〜`P2-10` を混在させた最小結合回帰を追加
- 完了条件
  - 追加回帰で `P2` の残存未実装 3 付随カテゴリの主要不具合再現が潰れていること
  - `P2` フェーズ完了基準として本節の 4 件がすべて `[x]` になっていること

#### 13.3.next P2 直近実行順

- 直近実行順
  1. `13.3.z` の `P2-08-01` から着手して timer 時刻基底を確定
  2. `13.3.z` の `P2-08-02` / `P2-08-03` を通して時間制御の基本経路を固定
  3. `13.3.z` の `P2-08-04` / `P2-08-05` を閉じる
  4. `13.3.w` → `13.3.v` → `13.3.u` の順で回帰連携を詰める
- 記録ルール
  - 各 `P2-08`〜`P2-10` の完了時に、本ファイルの該当 `[ ]` を `[x]` に更新
  - `13.3.u` の 4 項目完了後に `13.3` の `P2-11` を `[x]` 化
### 13.4 P3: 障害モデル（no-op 解消）

1. [x] P3-01 `supervision_stop` / `supervision_resume` / `supervision_restart` を実行器に接続する
2. [x] P3-02 `restart_source_with_backoff` の再起動遷移を実装する
3. [x] P3-03 `restart_flow_with_backoff` の再起動遷移を実装する
4. [x] P3-04 `restart_sink_with_backoff` の再起動遷移を実装する
5. [x] P3-05 `recover` の Failure/Error 振る舞いを再確認し、要素型 `Result` パスと分離する
6. [x] P3-06 `recover_with_retries` の最大再試行数と `fallback` 仕様を固定する
7. [x] P3-07 `kill switch` / `abort` 状態遷移を再起動・進捗 loop と整合化する
8. [x] P3-08 backoff 実行時刻の再現テストを追加する
9. [x] P3-09 Phase3 対象テストを固定し `./scripts/ci-check.sh` 対象範囲を更新する

### 13.5 P4: 動的制御・検証基盤

1. [x] P4-01 `UniqueKillSwitch` を実行器に接続し、`shutdown`/`abort` の影響を確定する
2. [x] P4-02 `SharedKillSwitch` を追加し、複数 stream で検証する
3. [x] P4-03 `MergeHub` / `BroadcastHub` を `Source` / `Sink` 実装へ接続する
4. [x] P4-04 `TestSource` / `TestSink` probe の最小 API を追加する
5. [x] P4-05 需要制御・失敗注入・完了検証のシナリオを追加する
6. [x] P4-06 fuzz 相当のストレス系テストを追加し順序崩れ検知を導入する
7. [x] P4-07 P4 対象テストを固定し `./scripts/ci-check.sh all` を実行する

### 13.6 未実装オペレーター実装タスクリスト（カテゴリ単位）

1. [ ] O1 Source 未実装カテゴリを一括実装する（31件中7件対応）
2. [ ] O2 Sink 未実装カテゴリを一括実装する（25件）
3. [ ] O3 Converter 未実装カテゴリを一括実装する（7件）
4. [ ] O4 File I/O 未実装カテゴリを一括実装する（2件）
5. [ ] O5 Simple 未実装カテゴリを一括実装する（30件）
6. [ ] O6 Sink/Source 合成カテゴリを一括実装する（2件）
7. [ ] O7 非同期系カテゴリを一括実装する（4件）
8. [ ] O8 Timer 系カテゴリを一括実装する（7件）
9. [ ] O9 Backpressure aware 系カテゴリを一括実装する（7件）
10. [ ] O10 ネスト化/フラット化系カテゴリを一括実装する（4件）
11. [ ] O11 時間制御系カテゴリを一括実装する（5件）
12. [ ] O12 Fan-in 系カテゴリを一括実装する（18件）
13. [ ] O13 Fan-out 系カテゴリを一括実装する（7件）
14. [ ] O14 watching status 系カテゴリを一括実装する（2件）
15. [ ] O15 actor interop 系カテゴリを一括実装する（8件）
16. [ ] O16 圧縮系カテゴリを一括実装する（4件）
17. [ ] O17 Error handling 系カテゴリを一括実装する（8件）

### 13.7 クロージング

1. [ ] C1 未対応総数を170台にしない（現状169）ことを確認し、各スプリント終了時に減算して更新する
2. [ ] C2 全タスク完了時に最終的な互換性受け入れ基準を再チェックする
3. [ ] C3 命名規則、lints、`./scripts/ci-check.sh all` を最終合格させる
