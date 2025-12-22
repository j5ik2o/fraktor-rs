# 要件ドキュメント

## 導入
本仕様は、Tokio 環境で動作する GossipTransport と Gossiper を追加し、クラスタの gossip を実ネットワークで駆動できるようにすることを目的とする。対象は `modules/cluster` の std 層と、動作確認用の examples である。

## 要件

### 要件1: Tokio GossipTransport の送受信
**目的:** クラスタ実装者として Tokio 環境で gossip の送受信を実現し、会員情報の同期を実ネットワークで検証したい。

#### 受け入れ条件
1. GossipOutbound の送信要求が起きたとき、Tokio GossipTransport は宛先 authority へ送信しなければならない。
2. 送信が失敗したならば、Tokio GossipTransport は送信失敗をエラーとして返さなければならない。
3. gossip 受信イベントが起きたとき、Tokio GossipTransport は送信元 authority と MembershipDelta の組を返さなければならない。
4. 受信キューが空の間、Tokio GossipTransport は空の結果を返し続けなければならない。

### 要件2: Tokio Gossiper のライフサイクル
**目的:** クラスタ運用者として gossip の開始と停止を制御し、実行状態を明確に管理したい。

#### 受け入れ条件
1. start が呼ばれたとき、Tokio Gossiper は gossip 処理を開始しなければならない。
2. stop が呼ばれたとき、Tokio Gossiper は gossip 処理を停止しなければならない。
3. 起動中の間、Tokio Gossiper は定期的に gossip の送受信処理を進め続けなければならない。
4. 開始処理に失敗したならば、Tokio Gossiper はエラーを返さなければならない。
5. 停止処理に失敗したならば、Tokio Gossiper はエラーを返さなければならない。

### 要件3: MembershipCoordinator 連携
**目的:** クラスタ運用者として gossip の結果がトポロジ更新として反映され、EventStream で観測できるようにしたい。

#### 受け入れ条件
1. gossip 受信データが処理されたとき、クラスタランタイムは MembershipCoordinator を更新しなければならない。
2. MembershipCoordinator がトポロジ更新を生成したとき、クラスタランタイムは ClusterEvent を EventStream へ通知しなければならない。
3. gossip outbound が生成されたとき、クラスタランタイムは GossipTransport を通じて送信しなければならない。
4. transport の送信が失敗したならば、クラスタランタイムはエラーを返さなければならない。

### 要件4: Tokio gossip サンプル
**目的:** 利用者として Tokio gossip の動作を再現し、クラスタ join/leave の挙動を確認したい。

#### 受け入れ条件
1. Tokio GossipTransport + Gossiper を含む場合、クラスタランタイムは `modules/cluster/examples` に実行可能な Tokio gossip サンプルを提供しなければならない。
2. サンプルが実行されたとき、サンプルは TopologyUpdated を 1 回以上確認し、成功終了しなければならない。
3. サンプルが実行されたとき、サンプルは 2 ノードの join/leave を検証しなければならない。

### 要件5: ビルド境界と検証
**目的:** 開発者として no_std を維持しつつ、std/Tokio で機能を利用できるようにしたい。

#### 受け入れ条件
1. std 機能を含む場合、クラスタランタイムは Tokio GossipTransport と Tokio Gossiper を使用可能にしなければならない。
2. std 機能を含まないならば、クラスタランタイムは Tokio 依存を要求してはならない。
3. テストが実行されたとき、Tokio GossipTransport と Tokio Gossiper の送受信およびライフサイクルを検証するテストは成功しなければならない。
4. no_std と std の両方でビルドが行われたとき、クラスタランタイムはビルドに成功しなければならない。
