## ADDED Requirements

### Requirement: inbound restart budget

std adaptor の association runtime は `RemoteConfig` の inbound restart budget を参照し、inbound loop の再起動を deadline-window budget で制限する SHALL。budget 超過時は無限 restart を行わず、観測可能な失敗として呼び出し元または runtime の error path に返す MUST。

#### Scenario: inbound restart 設定を runtime に渡す

- **WHEN** std adaptor が `RemoteConfig` から association runtime を構築する
- **THEN** inbound restart timeout と inbound max restarts が inbound loop の restart policy に渡される

#### Scenario: budget 内の inbound restart を許可する

- **WHEN** inbound loop が restart timeout window 内で inbound max restarts 以下の回数だけ失敗する
- **THEN** association runtime は inbound loop の restart を許可する

#### Scenario: budget 超過時に inbound restart を停止する

- **WHEN** inbound loop が restart timeout window 内で inbound max restarts を超えて失敗する
- **THEN** association runtime は追加 restart を行わず、失敗を観測可能な error path に返す

#### Scenario: restart window は monotonic time で判定する

- **WHEN** association runtime が inbound restart timeout window を評価する
- **THEN** `Instant` ベースの monotonic millis を使い、`SystemTime` などの wall clock に依存しない

### Requirement: advanced settings do not imply Pekko wire compatibility

std adaptor は `RemoteConfig` の large-message / compression advanced settings を参照可能にする SHALL。ただし、この change では Pekko Artery TCP framing、protobuf control PDU、compression table の byte-compatible な送受信を実装しない MUST。

#### Scenario: large-message 設定は wire codec を変更しない

- **WHEN** large-message destinations または outbound large-message queue size が設定されている
- **THEN** std adaptor は既存の fraktor-rs wire codec を維持し、Pekko Artery TCP framing を生成しない

#### Scenario: compression 設定は wire codec を変更しない

- **WHEN** compression settings が設定されている
- **THEN** std adaptor は compression table advertisement や Pekko protobuf control PDU を送信せず、設定値を後続 protocol 実装の入力として保持する
