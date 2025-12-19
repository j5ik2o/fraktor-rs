# 要件ドキュメント

## 導入
Pekko 互換のリモート通信を Rust/no_std ランタイムへ段階移植するため、まずはトランスポート抽象・Association 状態機械・メッセージ配送と観測基盤を整備し、後続の高機能リモーティングを安全に載せられる土台を定義する。

本仕様では Pekko リモートの骨格を形成する以下 7 コンポーネントを対象とする。
- **Remoting / RemoteTransport**: プロトコル選択と接続ライフサイクルを統括。
- **EndpointManager / EndpointRegistry**: Association/Quarantine 状態機械と遅延キュー管理を司る。
- **EndpointWriter / EndpointReader**: 送受信データのシリアライズ／再配送と優先度制御を担う。
- **EventPublisher / RemotingLifecycleEvent**: 状態変化やエラーを EventStream へ公開。
- **FailureDetector / RemoteWatcher**: ハートビート監視と Suspect/Quarantine 通知を実施。
- **RemotingFlightRecorder**: メトリクスとトレース相関 ID を収集。
- **RemoteActorRefProvider**: ActorSystem へ remoting を組み込み、Daemon/Transport を初期化。

## 要件

### 要件1: Remoting 構成
**目的:** 運用者として トランスポート差し替え可能な Remoting を実現し、 多様なターゲット間で一貫した接続ライフサイクル制御 を得たい。

#### 受け入れ条件
1. リモート設定が読み込まれたとき、Remoting はスキームと環境（std/no_std）に応じたトランスポート実装を選択しなければならない。
2. Remoting が稼働中の間、Remoting は開始・停止・バックプレッシャーフックを公開し続けなければならない。
3. サポートされないスキームが指定されたならば、Remoting は起動を拒否し、構成エラーを EventStream に通知しなければならない。
4. std 機能が有効な場合、Remoting は Tokio 互換の非同期ソケット境界を使用しなければならない。
5. Remoting は常に全送信ペイロードへ長さプリフィクス付きのフレーミングを適用しなければならない。
6. 開発者が Quickstart ガイドに従うとき、ドキュメントは ActorSystem 構築と Remoting Extension 初期化を結合したコード例を提示しなければならない。
7. Remoting Extension は SystemGuardian 領域へ子アクターを生成できる System API を提供されていなければならず、拡張は低レベル guardian 操作を直接行わなくてもよい。

### 要件2: EndpointManager と隔離状態
**目的:** ランタイム制御者として リモートエンドポイントの Association/Quarantine 遷移 を実現し、 障害時にもメッセージ順序と安全性を維持したい。

#### 受け入れ条件
1. 新しいエンドポイントへの接続要求が発生したとき、EndpointManager は Unassociated から Associating へ遷移し、ハンドシェイクペイロードを送出しなければならない。
2. リモート UID が未確定の間、EndpointRegistry は受信メッセージをハンドシェイク扱いにしてユーザートラフィックを遅延させ続けなければならない。
3. UID 不一致や隔離ルールが発動した場合、EndpointManager は Quarantined 状態へ遷移し、遅延キューを失敗理由付きで破棄しなければならない。
4. 手動復旧指示を受け取り隔離タイムアウトが経過したとき、EndpointManager は Connected へ復帰し、遅延メッセージを順序通りに再送しなければならない。
5. EndpointRegistry は常に最新の状態遷移時刻と理由を記録しなければならない。

### 要件3: EndpointWriter と優先制御
**目的:** ActorSystem として リモート経路への送受信をローカルと同一 API で扱えること を実現し、 シリアライズ一貫性と system message 優先度 を維持したい。

#### 受け入れ条件
1. リモート宛のアウトバウンドメッセージがスケジュールされたとき、EndpointWriter は ActorPath・UID とシリアライザマニフェストを含むペイロードへ変換しなければならない。
2. system メッセージがリモートアクター向けメールボックスに存在する間、EndpointWriter はユーザーメッセージより system メッセージを優先して送出し続けなければならない。
3. シリアライズに失敗したならば、EndpointWriter はメッセージを DeadLetter へ転送し、失敗内容を EventStream へ通知しなければならない。
4. reply_to が指定されている場合、EndpointWriter は復路 ActorPath メタデータをペイロードへ含めなければならない。
5. EndpointWriter は常に at-most-once 送達の前提で再送を利用者の制御に委ねなければならない。

### 要件4: EventPublisher と観測シグナル
**目的:** SRE として リモート基盤の状態を即時に観測・診断できること を実現し、 障害切り分けと自動復旧判断を迅速化したい。

#### 受け入れ条件
1. トランスポートまたは Association 状態が変化したとき、EventPublisher は RemotingLifecycleEvent を EventStream へ公開しなければならない。
2. ハートビートがしきい値を超えて欠損したならば、FailureDetector は該当エンドポイントを Suspect として EndpointManager へ通知しなければならない。
3. Remoting が有効な間、RemotingFlightRecorder は遅延キュー深さ・往復遅延・エラーレートなどのメトリクスを収集し続けなければならない。
4. トレース出力が有効な場合、RemotingFlightRecorder は送受信フレームへ相関 ID を付与しなければならない。
5. EndpointRegistry は常に全エンドポイントを状態別に要約するヘルススナップショット API を提供しなければならない。
