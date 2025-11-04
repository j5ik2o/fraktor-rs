## ADDED Requirements
### Requirement: Serializer 抽象を提供する
ランタイムは `modules/serializer-core` を通じて `no_std + alloc` 環境でも利用可能な `Serializer` 抽象を公開しなければならない (MUST)。また `std` feature が有効な場合は `modules/serializer-std` を介して追加実装を提供しなければならない (MUST)。

#### Scenario: AnyMessage をバイト列に変換できる
- **GIVEN** `serializer-core` が `bincode` または同等のバイナリ実装を登録している
- **WHEN** ランタイムが `AnyMessage` をリモート経路へ送信しようとする
- **THEN** メッセージはバージョン情報付きのバイト列としてエンコードされ、シリアライズ失敗時には recoverable なエラーが返される

#### Scenario: バックエンドを差し替えられる
- **GIVEN** デフォルトの `Serializer` 実装 (`bincode` など) が有効になっている
- **WHEN** 利用者が `ActorSystem` 構築時に別の `Serializer` を注入する
- **THEN** 既存 API を変更せずにエンコード/デコードが差し替わり、互換性チェックが実行される

#### Scenario: std 依存の JSON 実装を選択できる
- **GIVEN** `serializer-std` feature が有効で `serde_json` ベースの実装が登録されている
- **WHEN** 利用者が `ActorSystem` 構築時に JSON シリアライザを指名する
- **THEN** `AnyMessage` のエンコード/デコードが JSON に切り替わり、`no_std` 構成には影響を与えない

### Requirement: RemotePid から ActorRef を復元できる
ランタイムは `modules/remote-core` を通じて `RemotePid` 記述子を標準化し、システム間で `ActorRef` を復元可能にしなければならない (MUST)。

#### Scenario: RemotePid を解決してメッセージを送れる
- **GIVEN** `RemotePid` が `system_id`, `pid`, `name`, `request_id` を保持する
- **WHEN** 受信側 `ActorSystem` が `remote-core` の resolver に `RemotePid` を渡す
- **THEN** 対応する `ActorRef` が復元され、以降のメッセージがリモートアクターへ配送される

#### Scenario: 解決失敗時にフォールバックする
- **GIVEN** `RemotePid` が期限切れで resolver が見つけられない
- **WHEN** `ActorSystem` が `remote-core` の resolver を通じて復元を試みる
- **THEN** エラーが返却され、要求側に再解決もしくは再登録を促すイベントが通知される

### Requirement: Transport バッチングプロトコルを確立する
ランタイムは `modules/remote-core` でメッセージバッチを扱う `Transport` 抽象を提供し、`modules/remote-std` の `std` 依存実装を通じてネットワーク越しに安定配送できるようにしなければならない (MUST)。

#### Scenario: バッチ送信で往復通信を成立させる
- **GIVEN** `remote-core` がヘッダ・本文・メタデータで構成されたバッチ形式を規定し、`remote-std` の TCP 実装がそれを送受信できる
- **WHEN** 送信側が複数メッセージをまとめて送信する
- **THEN** 受信側は順序と `request_id` を保持したまま展開し、必要に応じて ACK を返す

#### Scenario: 再送が要求される
- **GIVEN** バッチの ACK がタイムアウトした
- **WHEN** `remote-std` の `Transport` 実装が再送ポリシーを評価する
- **THEN** 指定回数まで再送を行い、限界到達時にはエラーイベントを発火して上位に伝達する
