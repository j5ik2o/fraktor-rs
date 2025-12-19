# 要件ドキュメント

## 導入
ProtoActor-Go 互換の Cluster 型を fraktor-rs に統合し、拡張として起動・停止できるクラスタランタイムを提供する。

## 要件

### 要件1: クラスタ拡張の起動と終了
**目的:** 運用者としてクラスタノードやクライアントを安全に起動・停止し、失敗時も一貫した状態を維持したい。

#### 受け入れ条件
1. 当該サービスが `StartMember` を呼び出されたとき、Clusterサービスは Remote・ClusterProvider・拡張内部状態を初期化し、いずれかが失敗した場合は起動を中止して理由を返さなければならない。
2. 当該サービスが `StartClient` を呼び出されたとき、Clusterサービスは Remote を起動し、IdentityLookup をクライアントモードで初期化しなければならない。
3. If ClusterProvider の起動が失敗した場合、Clusterサービスは発生理由を含むエラーを返し、開始済みサブシステムを停止しなければならない。
4. While graceful shutdown が要求されている間、Clusterサービスは Gossip・IdentityLookup・MemberList を順に停止し、Remote を最後に停止し続けなければならない。
5. The Clusterサービス shall 起動と停止のアドレスおよびモード（member/client）をロガーへ記録しなければならない。

### 要件2: Kind 登録とアイデンティティ初期化
**目的:** 開発者としてクラスタの Kind 定義を一貫して登録し、IdentityLookup が適切なモードでセットアップされるようにしたい。

#### 受け入れ条件
1. 当該サービスがメンバー起動時に Kind 設定を読み込んだとき、Clusterサービスは各 Kind をビルドし、TopicActorKind が未登録なら自動登録しなければならない。
2. When 未登録の Kind が `GetClusterKind` で要求されたとき、Clusterサービスは無効な Kind であることを記録し、空結果を返さなければならない。
3. 当該サービスが member/client モードで起動するとき、Clusterサービスは登録済み Kind 一覧を渡して IdentityLookup.Setup を呼び出さなければならない。
4. While Kind 構成が変更されていない間、Clusterサービスは Kind を再ビルドせず初期化時の構成を保持し続けなければならない。
5. The Clusterサービス shall VirtualActorCount を登録済み Kind から集計できなければならない。

### 要件3: メンバーシップとトポロジ連動
**目的:** クラスタ管理者としてトポロジ変化を PID キャッシュやメトリクスに即時反映し、古い参照を残さないようにしたい。

#### 受け入れ条件
1. When ClusterTopology イベントで離脱ノードが通知されたとき、Clusterサービスは当該メンバーの PID キャッシュを削除しなければならない。
2. When ClusterTopology イベントを受信したとき、Clusterサービスは metrics 有効時にクラスタメンバー数メトリクスを更新しなければならない。
3. When メンバー起動が完了したとき、Clusterサービスは MemberList のトポロジコンセンサスを初期化しなければならない。
4. If Gossip の開始が失敗した場合、Clusterサービスは起動処理を停止し、致命的エラーとして報告しなければならない。
5. While トポロジハッシュが変化しない間、Clusterサービスは追加の ClusterTopology イベントを発火しないよう抑制し続けなければならない。

### 要件4: Gossip・PubSub・通信基盤の起動
**目的:** SRE として Gossip と PubSub を確実に起動し、リモート送達経路が安定して確立されるようにしたい。

#### 受け入れ条件
1. When Clusterサービスがメンバーとして起動したとき、Clusterサービスは Gossip を開始し、起動失敗時はクラスタ起動を中断しなければならない。
2. When Clusterサービスが起動したとき、Clusterサービスは PubSub を起動し、TopicActorKind 登録後に購読受付を有効化しなければならない。
3. If Gossip または PubSub のいずれかが起動中にエラーを返した場合、Clusterサービスは理由を含めて起動失敗を通知しなければならない。
4. While Gossip が稼働している間、Clusterサービスは最新トポロジを Gossip 経由で配布し続けなければならない。
5. The Clusterサービス shall BlockList に登録されたメンバー一覧を取得可能でなければならない。

### 要件5: 観測性とメトリクス
**目的:** 観測者としてクラスタの状態やカウント指標を取得し、運用メトリクスとログを一元的に確認したい。

#### 受け入れ条件
1. When ActorSystem の metrics 設定が有効なとき、Clusterサービスは ClusterMetrics を初期化しメンバー数や仮想アクター数を観測可能にしなければならない。
2. When ActorSystem の metrics 設定が無効なとき、Clusterサービスはメトリクスを初期化せず通常動作を継続しなければならない。
3. While metrics が有効な間、Clusterサービスは ClusterTopology 更新ごとにメンバー数メトリクスを更新し続けなければならない。
4. When VirtualActorCount が要求されたとき、Clusterサービスは全 Kind から集計した結果を返さなければならない。
5. If メトリクスが無効な状態でメトリクス値が要求された場合、Clusterサービスは未収集であることを通知しなければならない。
