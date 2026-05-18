## ADDED Requirements

### Requirement: RemoteDeploymentPdu の wire 表現

wire format は remote actor deployment 用の `RemoteDeploymentPdu` を encode / decode しなければならない（MUST）。PDU は create request、create success、create failure を表現し、すべての variant は request/response を対応付ける correlation id を持たなければならない（MUST）。

create request は target parent path、child name、deployable factory id、serialized factory payload metadata、sender node metadata を含む SHALL。serialized factory payload は actor-core serialization の serializer id、manifest、payload bytes として保持し、raw `Props` や closure 表現を含んではならない（MUST NOT）。

#### Scenario: create request round-trips

- **WHEN** target parent path、child name、factory id、serializer id、manifest、payload bytes、correlation id を持つ create request を encode して decode する
- **THEN** decode 後の `RemoteDeploymentPdu` は同じ field values を保持する

#### Scenario: create success round-trips

- **WHEN** created actor canonical path と correlation id を持つ create success を encode して decode する
- **THEN** decode 後の success response は同じ created actor path と correlation id を保持する

#### Scenario: create failure round-trips

- **WHEN** failure code、reason、correlation id を持つ create failure を encode して decode する
- **THEN** decode 後の failure response は同じ failure code、reason、correlation id を保持する

#### Scenario: malformed deployment payload is rejected

- **WHEN** deployment frame が serializer metadata または payload length を欠落した状態で decode される
- **THEN** decode は `WireError::InvalidFormat` または `WireError::Truncated` を返す

### Requirement: RemoteDeploymentPdu は actor envelope と混同しない

remote deployment create request/response は user message `EnvelopePdu` として encode してはならない（MUST NOT）。wire format は deployment PDU を actor delivery とは別の frame kind または control subkind として識別できなければならない（MUST）。

#### Scenario: deployment frame is not delivered as user envelope

- **WHEN** TCP reader が remote deployment create request frame を受信する
- **THEN** frame は actor user message delivery へ渡されない
- **AND** std deployment daemon の request handling path へ渡される

#### Scenario: deployment response is not delivered as user envelope

- **WHEN** TCP reader が remote deployment create success または create failure frame を受信する
- **THEN** frame は actor user message delivery へ渡されない
- **AND** origin provider の pending response handling path へ渡される
