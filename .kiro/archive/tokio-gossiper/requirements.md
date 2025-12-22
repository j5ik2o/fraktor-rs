# 要件ドキュメント

## 導入

本仕様は、protoactor-go 互換の Gossiper を Rust + Tokio で実装するための要件を定義する。既存の no_std 対応 GossipEngine を Tokio ランタイム上で駆動し、クラスタメンバー間でメンバーシップ状態を定期的に拡散・同期するコンポーネントを構築する。protoactor-go の `gossiper.go`、`gossip_actor.go`、`gossip_state_management.go` を参考にし、LWW (Last-Writer-Wins) + SequenceNumber ベースの状態マージ戦略を採用する。

## 要件

### 要件1: TokioGossiper 構造体と Gossiper トレイト実装

**目的:** クラスタ開発者として、Tokio ランタイム上で動作する Gossiper 実装を利用し、クラスタメンバー間の状態同期を実現したい。

#### 受け入れ条件

1. TokioGossiper は既存の Gossiper トレイトを実装しなければならない
2. TokioGossiper の `start()` が呼び出されたとき、TokioGossiper は内部のゴシップループを開始しなければならない
3. TokioGossiper の `stop()` が呼び出されたとき、TokioGossiper は内部のゴシップループを graceful に停止しなければならない
4. TokioGossiper の `start()` が既に開始済みの状態で呼び出された場合、TokioGossiper はエラーを返さなければならない
5. TokioGossiper の `stop()` が開始前または既に停止済みの状態で呼び出された場合、TokioGossiper はエラーを返さなければならない
6. TokioGossiper は Send + Sync を満たし、複数スレッドから安全に参照可能でなければならない

### 要件2: 定期的なゴシップ送信ループ

**目的:** クラスタ開発者として、設定可能な間隔でゴシップ状態を自動的にピアに拡散し、クラスタ全体の状態収束を実現したい。

#### 受け入れ条件

1. ゴシップループが開始されている間、TokioGossiper は設定された GossipInterval に従って定期的にゴシップを送信しなければならない
2. 各ゴシップ間隔において、TokioGossiper はハートビート状態を更新しなければならない
3. 各ゴシップ間隔において、TokioGossiper は期限切れハートビートを持つメンバーをブロックしなければならない
4. 各ゴシップ間隔において、TokioGossiper は gracefully left したメンバーをブロックしなければならない
5. ゴシップ送信は tokio::time::interval を使用して実装しなければならない
6. GossipInterval は設定により変更可能でなければならない（デフォルト値は protoactor-go と互換）

### 要件3: GossipEngine との統合

**目的:** クラスタ開発者として、既存の no_std GossipEngine を活用し、コードの再利用性を高めたい。

#### 受け入れ条件

1. TokioGossiper は内部で既存の GossipEngine を保持し、状態管理を委譲しなければならない
2. ゴシップ状態の disseminate 要求が発生したとき、TokioGossiper は GossipEngine の disseminate メソッドを呼び出しなければならない
3. ピアからのゴシップ受信が発生したとき、TokioGossiper は GossipEngine の apply_incoming メソッドを呼び出しなければならない
4. GossipEngine が GossipEvent を生成したとき、TokioGossiper はそれを EventStream に発行しなければならない
5. TokioGossiper は GossipEngine の状態（Diffusing/Reconciling/Confirmed）を外部から参照可能にしなければならない

### 要件4: 状態の取得・設定操作

**目的:** クラスタ開発者として、ゴシップ経由で共有される状態を取得・設定し、クラスタ全体で一貫した情報を共有したい。

#### 受け入れ条件

1. TokioGossiper の `get_state(key)` が呼び出されたとき、TokioGossiper は指定されたキーに対応する全メンバーの状態を返さなければならない
2. TokioGossiper の `set_state(key, value)` が呼び出されたとき、TokioGossiper は自ノードの状態を更新し、次回のゴシップで拡散しなければならない
3. 状態設定時、TokioGossiper は SequenceNumber をインクリメントしなければならない
4. 状態設定時、TokioGossiper はローカルタイムスタンプを記録しなければならない
5. TokioGossiper の `set_state_request(key, value)` が呼び出されたとき、TokioGossiper は設定完了まで待機する同期版 API を提供しなければならない

### 要件5: 状態マージ（LWW + SequenceNumber）

**目的:** クラスタ開発者として、複数ノードからの状態更新を一貫した方法でマージし、eventual consistency を実現したい。

#### 受け入れ条件

1. リモート状態を受信したとき、TokioGossiper は SequenceNumber を比較して新しい状態のみを適用しなければならない
2. リモート状態の SequenceNumber がローカルより大きい場合、TokioGossiper はローカル状態を上書きしなければならない
3. リモート状態の SequenceNumber がローカル以下の場合、TokioGossiper はリモート状態を無視しなければならない
4. 状態マージ時、TokioGossiper は LocalTimestamp を現在時刻に更新しなければならない
5. 状態マージにより変更が発生したとき、TokioGossiper は GossipUpdate イベントを生成しなければならない

### 要件6: ハートビート管理

**目的:** クラスタ開発者として、メンバーの生存状態を監視し、障害ノードを検出・隔離したい。

#### 受け入れ条件

1. ゴシップ間隔ごとに、TokioGossiper は HeartbeatKey として MemberHeartbeat 状態を設定しなければならない
2. MemberHeartbeat には ActorStatistics（各 Kind ごとのアクター数）を含めなければならない
3. HeartbeatExpiration を超えてハートビートを受信していないメンバーを検出したとき、TokioGossiper はそのメンバーをブロックリストに追加しなければならない
4. HeartbeatExpiration は設定により変更可能でなければならない
5. 自ノードのハートビートはブロック対象から除外しなければならない

### 要件7: ClusterTopology イベントへの対応

**目的:** クラスタ開発者として、クラスタトポロジの変更に応じてゴシップ対象を更新し、正確な状態拡散を維持したい。

#### 受け入れ条件

1. ClusterTopology イベントを受信したとき、TokioGossiper は内部のピアリストを更新しなければならない
2. ClusterTopology で新規メンバーが参加したとき、TokioGossiper はそのメンバーをゴシップ対象に追加しなければならない
3. ClusterTopology でメンバーが離脱したとき、TokioGossiper はそのメンバーをゴシップ対象から除外しなければならない
4. TokioGossiper は EventStream を購読して ClusterTopology イベントを受信しなければならない

### 要件8: ネットワーク通信

**目的:** クラスタ開発者として、ゴシップメッセージをクラスタメンバー間で送受信し、状態を伝播したい。

#### 受け入れ条件

1. TokioGossiper はゴシップリクエストをリモートピアに送信できなければならない
2. TokioGossiper はゴシップリクエストを受信し、応答を返すことができなければならない
3. ゴシップリクエスト送信時、TokioGossiper は GossipRequestTimeout を超えた場合にタイムアウトエラーを返さなければならない
4. GossipRequestTimeout は設定により変更可能でなければならない
5. GossipFanOut（同時送信先数）は設定により変更可能でなければならない
6. GossipMaxSend（1回の送信での最大状態数）は設定により変更可能でなければならない

### 要件9: Graceful Shutdown

**目的:** クラスタ開発者として、ノード停止時に適切にゴシップを終了し、クラスタの安定性を維持したい。

#### 受け入れ条件

1. `stop()` が呼び出されたとき、TokioGossiper は進行中のゴシップ送信の完了を待機しなければならない
2. シャットダウン時、TokioGossiper は GracefullyLeftKey として自ノードの離脱状態を設定しなければならない
3. シャットダウン後、TokioGossiper は新規のゴシップ要求を受け付けてはならない
4. シャットダウンが完了したとき、TokioGossiper は全ての内部リソースを解放しなければならない

### 要件10: ブロックメンバー管理

**目的:** クラスタ開発者として、問題のあるメンバーをブロックし、クラスタの健全性を保ちたい。

#### 受け入れ条件

1. TokioGossiper はブロックされたメンバーのリストを取得できなければならない
2. ブロックされたメンバーに対して、TokioGossiper はゴシップを送信してはならない
3. GracefullyLeftKey を持つメンバーを検出したとき、TokioGossiper はそのメンバーをブロックリストに追加しなければならない
4. 既にブロック済みのメンバーを再度ブロックしようとした場合、TokioGossiper は何もしなければならない

### 要件11: コンセンサスチェック（オプション）

**目的:** クラスタ開発者として、特定の状態についてクラスタ全体で合意が取れているかを確認したい。

#### 受け入れ条件

1. TokioGossiper の `register_consensus_check(key, extractor)` が呼び出されたとき、TokioGossiper はコンセンサスチェッカーを登録しなければならない
2. コンセンサスチェッカーは指定されたキーの値について全メンバーが同一かどうかを判定しなければならない
3. コンセンサスが達成されたとき、TokioGossiper は登録されたコールバックを呼び出さなければならない
4. TokioGossiper の `remove_consensus_check(id)` が呼び出されたとき、TokioGossiper は指定されたコンセンサスチェッカーを削除しなければならない

### 要件12: 設定と初期化

**目的:** クラスタ開発者として、Gossiper の動作を要件に応じてカスタマイズしたい。

#### 受け入れ条件

1. TokioGossiperConfig は以下の設定項目を含まなければならない:
   - GossipInterval（ゴシップ間隔）
   - GossipRequestTimeout（リクエストタイムアウト）
   - GossipFanOut（同時送信先数）
   - GossipMaxSend（最大送信数）
   - HeartbeatExpiration（ハートビート期限）
   - GossipActorName（オプション、デフォルト: "gossip"）
2. TokioGossiper は TokioGossiperConfig と Cluster 参照から構築可能でなければならない
3. 設定値が不正な場合、TokioGossiper の構築は検証エラーを返さなければならない
