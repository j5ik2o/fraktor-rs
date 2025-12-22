# 要件ドキュメント

## 導入
Pekko/Classic と同等に、ActorRef が生成された瞬間から論理パスと物理アドレス（authority 付き canonical URI）を保持し、シリアライズ時に自動でリモート可の参照へ変換される UX を fraktor-rs に導入する。これにより利用者はローカルで取得した ActorRef をそのままリモートへ送信でき、明示的な Provider 呼び出しや手動の authority 注入を不要にする。

## 要件

### 要件1: Canonical パス付与
**目的:** 利用者として、ActorRef が常に自身の物理アドレスを含む canonical URI を公開し、リモート送信時に追加操作なしで到達可能にしたい。

#### 受け入れ条件
1. When `RemotingConfig` に canonical host/port が設定されているとき、ActorRef システムは scheme `fraktor.tcp` と host:port を含む canonical パスを公開しなければならない。
2. While `RemotingConfig` が未設定の間、ActorRef システムは authority を含まないパスを公開し続けローカル参照として扱い続けなければならない。
3. When UID が割り当てられているとき、ActorRef システムは canonical URI に UID フラグメントを付与しなければならない。
4. If canonical host または port が欠落している場合、ActorRef システムは canonical 化を拒否し、公開 API を通じて構成エラーを返さなければならない。
5. When guardian や子セグメントが指定されているとき、ActorRef システムはそれらのセグメントを保持したまま canonical URI を生成しなければならない。
6. When bind アドレスと公開 (advertise) アドレスが異なる場合、ActorRef システムは canonical URI 生成に公開アドレスを用いなければならない。

### 要件2: シリアライズとデシリアライズ
**目的:** 運用者として、ActorRef のシリアライズ／復元で常に正しいアドレス情報が付与され、リモート環境でも解決可能であることを保証したい。

#### 受け入れ条件
1. When ActorRef を `SerializationExtension` でシリアライズし、`TransportInformation` が無く `RemotingConfig` が存在するとき、SerializationExtension は canonical address をシリアライズ済みパスの先頭に付与しなければならない。
2. While `TransportInformation` がスタックに存在する間、SerializationExtension はシステム既定よりも当該アドレスを優先してシリアライズ出力を行わなければならない。
3. If authority を含むシリアライズ済みパスに対応するプロバイダが登録されていない場合、システムは解決エラーを返し、診断イベントを公開しなければならない。
4. When transport information も remoting config も利用できないとき、SerializationExtension は `local://` プレフィックス付きパスをフォールバックとして出力しなければならない。
5. While ActorRef 型のメタデータ（例: reply_to フィールド）をシリアライズする間、SerializationExtension は authority と UID を欠落させず保持しなければならない。

### 要件3: ActorRef フィールドの経路一貫性
**目的:** 開発者として、メッセージに含まれる任意の ActorRef 型フィールドが自動的にリモート到達可能な形に正規化され、往復の通信経路が一貫して動作することを望む。

#### 受け入れ条件
1. When 送信メッセージが authority を持たない ActorRef 型フィールドを含み canonical host/port が既知のとき、リモーティングパイプラインは送信前にその ActorRef へ authority を補完しなければならない。
2. When 受信メッセージが authority 付き ActorRef 型フィールドを含むとき、リモーティングパイプラインは以降の通信をローカル配送ではなく対応するリモートトランスポート経由でルーティングしなければならない。
3. If ActorRef 型フィールドに含まれる authority が隔離状態の場合、リモーティングパイプラインは配送を拒否し、隔離理由を EventStream に公開しなければならない。
4. While その authority へのアソシエーションハンドシェイクが未完了の間、リモーティングパイプラインは配送を接続成立まで遅延させるか、再試行可能エラーで失敗を返さなければならない。

### 要件4: ActorRef 解決ファサード
**目的:** 利用者として、スキームやプロバイダ実装を意識せず ActorPath から ActorRef を取得し、ローカル／リモートを同一 API で扱いたい。

#### 受け入れ条件
1. The ActorRef 解決 API は authority の有無を問わず ActorPath を受け取り、ActorRef もしくは記述的エラーを返さなければならない。
2. When authority が欠落し remoting config に canonical host/port があるとき、解決 API はプロバイダ探索前にローカル canonical authority を注入しなければならない。
3. While 複数の actor-ref provider が登録されている間、解決 API は ActorPath の scheme に一致するプロバイダを選択しなければならない。
4. If scheme が未知または対応プロバイダが存在しない場合、解決 API は unsupported-scheme エラーを返し、診断イベントを発行しなければならない。
5. When scheme がローカル配送を示すとき、解決 API はリモーティングトランスポートを経由せずローカル ActorRef を返さなければならない。

### 要件5: ドキュメントとサンプルの更新
**目的:** 学習者として、Quickstart やガイドを参照したときに新しい自動 canonical 化の使い方が明確で、手動 Provider 呼び出しが不要であると理解したい。

#### 受け入れ条件
1. When Tokio TCP Quickstart を実行するとき、ドキュメントはローカル取得 ActorRef をそのまま使ってリモート送信するサンプルコードを示し、明示的な provider 呼び出しを含めてはならない。
2. Where Remoting の feature flag を列挙する箇所では、remoting config が存在すれば canonical address 注入が自動で行われる旨を明記しなければならない。
3. If 利用者が必要な feature を有効にしてガイド手順に従った場合、手動の authority 注入や provider ルックアップ無しで例が end-to-end で動作しなければならない。
