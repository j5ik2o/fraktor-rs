# Implementation Plan

- [ ] 1. RFC2396 URI 解析レイヤーを確立する
  - 汎用 URI を AST 化し、ActorPath から独立した RFC2396 準拠の検証ステージを提供する。
  - percent decode と authority 検証を共通ユーティリティへ集約し、remoting など他機能でも再利用できる形にまとめる。
  - _Requirements: R2.1, R2.3, R2.5_

- [ ] 1.1 汎用 URI AST とスキャナを構築する
  - スキーム・authority・パス・クエリ・フラグメントを階層構造へ分解するロジックを実装する。
  - スキーム名の正規化と許可スキームのチェックを共通ユーティリティへ集約し、ActorPath 層へ構造体として渡す。
  - RFC2396 違反を種類別に識別できるエラー記述を整備する。
  - _Requirements: R2.1, R2.3_

- [ ] 1.2 パーセントエンコードとホスト検証を強化する
  - `%HH` 解析の状態機械を実装し、不正なバイト列を検出した時点で失敗させる。
  - ASCII/IPv4/IPv6 のホスト表記を許容しつつ、未対応フォーマットを正しく拒否する。
  - URI 層の検証結果を ActorPath 固有エラーへマッピングするための変換ヘルパを用意する。
  - _Requirements: R1.2, R2.5_

- [ ] 1.3 URI レイヤーのユニット／プロパティテストを整備する
  - Pekko/ProtoActor 由来の既知ケースを golden data として追加し、解析結果を固定化する。
  - ランダム入力で parse→format 往復のヒューリスティック検証を行い、例外発生時のエラー分類を確認する。
  - ログ／トレースで利用しやすいエラーメッセージ表現を検証する。
  - _Requirements: R2.1, R2.5_

- [ ] 2. ActorPath 値オブジェクトと正規化を確立する
  - Guardian ルート、セグメント検証、UID 取り扱いを値オブジェクトで一貫管理し、Formatter/Parser と連携させる。
  - 等価性と canonical URI 生成を deterministic に保ち、DeathWatch／ログ出力で同じ表現を再利用できるようにする。
  - _Requirements: R1.1, R1.2, R1.3, R1.4, R1.5, R1.6, R3.1, R3.2_

- [ ] 2.1 Guardian パスとセグメント検証を実装する
  - 設定から受け取った guardian 種別に応じて `/system` または `/user` を常に先頭へ挿入するロジックを組み込む。
  - セグメント文字種の検証と `$` 始まり予約語の拒否を実装し、元の大小文字を保持する。
  - 相対演算子 `..` が guardian より上位へ遡らないよう境界チェックを行う。
  - _Requirements: R1.2, R1.3, R1.5, R1.6, R2.2_

- [ ] 2.2 Canonical 表現と UID 一貫性を実現する
  - authority 有無に応じて `pekko://system@host:port/path` と `pekko://system/path` の双方を生成できる Formatter を整える。
  - UID サフィックスをパス構造と分離し、等価判定やハッシュでは無視しつつ表示では保持する。
  - 子パス生成時に親セグメントを再検証せず決定的に連結できるようハンドル情報を活用する。
  - _Requirements: R1.1, R1.4, R3.1, R3.2, R3.4_

- [ ] 2.3 Formatter／Parser のプロパティテストを追加する
  - `format(parse(x)) == canonical(x)` を多様な入力で検証するプロパティテストを実装する。
  - 大文字小文字維持や UID 温存を確認する golden cases を整備する。
  - Validator が拒否したケースのエラー内容を DeadLetter／ログで可視化できるよう asserts を用意する。
  - _Requirements: R1.1, R1.4, R1.5_

- [ ] 3. ActorSelection 解決と相対操作を強化する
  - 相対パス解決や親子連結を ActorPath 値オブジェクトベースで行い、決定的なツリー遷移を保証する。
  - Authority 未解決時に Resolver が deferred 処理へ委譲できるようハンドオフを整える。
  - _Requirements: R2.1, R2.2, R2.4, R3.4_

- [ ] 3.1 相対選択と子パス合成を実装する
  - ActorSelection からの `..`／`.`／ワイルドカードを解釈し、guardian を越えないよう保護する。
  - 子パス生成で親の検証をスキップしつつ、Segment 連結後の canonical URI を即座に得られるようにする。
  - Selection 失敗時のエラーを DeadLetter と EventStream へルーティングする。
  - _Requirements: R2.2, R3.4_

- [ ] 3.2 Authority 未解決時の遅延配送を組み込む
  - Resolver が authority 解決結果を照会し、存在しない場合は deferred キューへメッセージを積む。
  - 未解決 authority を Remote 管理層へ通知し、後続の状態遷移が起きた際に再配送できるフックを用意する。
  - Deferred 状態での監視／ログ出力を整え、利用者が遅延理由を追跡できるようにする。
  - _Requirements: R2.4, R4.1_

- [ ] 3.3 Resolver のシナリオテストを追加する
  - 正常／異常系の相対パス解決シナリオを追加し、`..` 超過や guardian 越えの失敗を検証する。
  - Authority 未解決と接続済みの両シナリオで deferred→配送完了の流れを確認する。
  - Selection 結果が Remoting/DeathWatch で共有されることを integration テストで保証する。
  - _Requirements: R2.1, R2.2, R2.4_

- [ ] 4. ActorPathRegistry と UID 予約ポリシーを実装する
  - PID→canonical path のキャッシュ、UID 予約、再利用判定を司るレジストリを導入する。
  - SystemState／ActorRef から統一 API で参照できるようにし、再生成判定を deterministic にする。
  - _Requirements: R3.1, R3.2, R3.3, R3.4, R3.6, R3.7_

- [ ] 4.1 パスハンドルと等価判定キャッシュを構築する
  - PID から canonical URI と UID 無視ハッシュを取得できるハンドル構造を定義する。
  - ActorRef レベルでの一意性チェックを実装し、既存参照との衝突を防ぐ。
  - Registry が child 生成や Resolver と共有できるようトレイト API を整備する。
  - _Requirements: R3.1, R3.2, R3.4, R3.6_

- [ ] 4.2 UID 予約と隔離期間ポリシーを組み込む
  - UID 予約テーブルに期限付きエントリを追加し、DeathWatch からの通知で延長／解放を制御する。
  - Remoting 設定から隔離期間を受け取り、デフォルト 5 日と API 上書きを切り替えられるようにする。
  - 予約状態にある UID へ再生成要求が来た場合のエラー伝搬を整える。
  - _Requirements: R3.3, R3.6, R3.7_

- [ ] 4.3 SystemState／ActorRef 連携テストを追加する
  - 一時アクター登録や PID 復元のシナリオでレジストリが正しい canonical URI を返すことを検証する。
  - UID 解放タイミングが DeathWatch 経由で伝搬する統合テストを追加する。
  - レジストリが複数スレッドから安全に利用できるかを stress テストで確認する。
  - _Requirements: R3.1, R3.3, R3.6_

- [ ] 5. Remote authority 状態管理と quarantining を実装する
  - Authority 未解決／接続／隔離の状態機械を整備し、Deferred キューと EventStream 通知を一体化する。
  - InvalidAssociation の発火条件と再解決サイクルを Remoting 設定と同期させる。
  - _Requirements: R2.4, R3.5, R4.1, R4.2, R4.3, R4.4_

- [ ] 5.1 状態遷移と deferred キューを実装する
  - Authority ごとの状態を保持し、未解決→接続→未解決の往復や接続→隔離の遷移を deterministic に定義する。
  - Deferred キューへ追加／flush する API を提供し、Resolver からの委譲を受け付ける。
  - 接続確立時に保留メッセージを FIFO で再配送する挙動を実装する。
  - _Requirements: R2.4, R4.1, R4.2_

- [ ] 5.2 Quarantine と InvalidAssociation 処理を実装する
  - InvalidAssociation 受信や期間超過失敗時に隔離へ遷移し、新規配送を拒否するロジックを実装する。
  - 隔離中の要求へは送信者単位で InvalidAssociation を返し、監視者が即座に把握できるようにする。
  - Remoting 設定の隔離期間をもとに解除時刻を計算し、期限まで状態を維持する。
  - _Requirements: R3.5, R4.3, R4.4_

- [ ] 5.3 EventStream 連携と手動解除パスを検証する
  - 状態遷移イベントを EventStream へ発行し、監視者が Unresolved/Connected/Quarantine を観測できるようにする。
  - 手動解除 API を通じて隔離から接続へ即時遷移させるパスをテストする。
  - Remoting 設定上の override 値が正しく適用されるか統合テストで確認する。
  - _Requirements: R4.2, R4.3, R4.4_

- [ ] 6. システム統合とリグレッション検証を行う
  - ActorSystemConfig／RemotingConfig から新機能を構築物へ供給し、ユーザ API の破壊的変更を整理する。
  - end-to-end テストと CI スクリプト実行で R1〜R4 を横断的に保証する。
  - _Requirements: R1.1, R1.3, R2.4, R3.3, R3.7, R4.1, R4.2, R4.3, R4.4_

- [ ] 6.1 Config ビルダーと API 表面を更新する
  - ActorSystemConfig に system 名・guardian 種別・デフォルト authority を設定するフローを追加する。
  - RemotingConfig から quarantine duration や canonical host 情報を注入するパスを整備する。
  - 新しい設定値を利用者が確認できるよう初期化ログやデバッグ情報を整える。
  - _Requirements: R1.3, R3.7_

- [ ] 6.2 End-to-end テストで ActorPath の互換性を確認する
  - 複数のローカル・リモート ActorPath に対し format→parse 往復で Pekko 互換 URI が得られることを検証する。
  - Authority 未解決／接続／隔離シナリオを再現し、Deferred キューや InvalidAssociation の挙動を確認する。
  - DeathWatch／ログ／ActorSelection から取得した URI が一致することを統合テストで証明する。
  - _Requirements: R1.1, R1.2, R1.3, R1.4, R1.5, R1.6, R2.1, R2.2, R2.3, R2.4, R2.5, R3.1, R3.2, R3.3, R3.4, R3.5, R3.6, R4.1, R4.2, R4.3, R4.4_

- [ ] 6.3 CI スクリプトとクロスターゲット検証を更新する
  - `scripts/ci-check.sh` の関連ジョブを更新し、新規テストスイートを no_std/std/embedded 両方で走らせる。
  - 追加した feature flag や config を cargo features／環境変数と整合させる。
  - リグレッションを防ぐための最小限の fuzz シードやベンチマークを登録する。
  - _Requirements: R1.1, R1.2, R1.3, R1.4, R1.5, R1.6, R2.1, R2.2, R2.3, R2.4, R2.5, R3.1, R3.2, R3.3, R3.4, R3.5, R3.6, R3.7, R4.1, R4.2, R4.3, R4.4_
