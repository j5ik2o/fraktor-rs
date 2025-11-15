# remote-core 設計メモ

## 1. 目的とスコープ
Pekko/Akka の remoting と同様に、ActorSystem 間でメッセージを透過的に伝送できる `remote-core` を fraktor に導入する。Mailbox/Scheduler/Serialization の既存基盤を活かしつつ、no_std/STD 両対応での接続維持・障害復旧を可能にする。ここでは Actor ベースでリモートクライアント/サーバを構築する方針と、その周辺モジュールの配置を整理する。

## 2. 全体アーキテクチャ
```
sender Actor
  └─> SystemMailbox
      └─> RemoteBridgeActor (client)  ※ /system/remoting/client/*
          └─> TransportAdapter (TCP/QUIC/カスタム)
              └─> RemoteBridgeActor (server)  ※ /system/remoting/server/*
                  └─> SystemMailbox
                      └─> receiver Actor
```
- RemoteBridgeActor は ActorSystem Guardian 配下に配置し、`SupervisorStrategy` で再起動・バックオフを統一管理する。
- TransportAdapter は最小限の I/O を担当し、メッセージ再送・ACK・心拍などは Actor ロジック側で扱う。
- すべてのリモートフレームは `Serialization` モジュールでエンコードし、`Scheduler` を利用して心拍・タイムアウト・再送を行う。

## 3. モジュール構成草案
| モジュール | 役割 | 備考 |
| --- | --- | --- |
| `modules/actor-core/src/remote/bridge_actor.rs` | RemoteBridgeActor 実装。EndpointWriter/Reader 相当で、接続状態と送受信キューを管理。 | System Guardian 配下でスーパービジョン |
| `modules/actor-core/src/remote/transport_adapter.rs` | TCP/QUIC など実トランスポートを抽象化した adapter。非同期 I/O or no_std HAL へのブリッジ。 | Transport 切替を trait 化 |
| `modules/actor-core/src/remote/handshake.rs` | 握手プロトコル、UID 交換、機能ネゴシエーション。 | 心拍/タイムアウトもここに集約 |
| `modules/actor-core/src/remote/endpoint_registry.rs` | リモートノード単位の状態（Connected/Quarantine/Pending）を保持。 | 既存 RemoteAuthorityManager と連携 |
| `modules/actor-core/src/remote/backpressure.rs` | ネットワーク出力のレート制御、再送キュー。 | Scheduler + Metrics と連動 |

## 4. Actor ベース実装の利点
1. **スーパービジョン**: Guardian 配下で RemoteBridgeActor を動かすことで、接続失敗時に BackoffSupervisor（今後実装予定）や既存 `SupervisorStrategy` をそのまま適用できる。  
2. **Mailbox/Scheduler 再利用**: 心拍や再送、再試行を Scheduler で管理でき、Mailbox 経由で優先度制御（SystemMailbox の高優先度）を得られる。  
3. **障害隔離**: ノードごと・接続ごとに Actor を分けることで、一部ノード障害が他ノードへ波及しにくく、再起動も局所化できる。  
4. **テスト容易性**: Actor として実装することで、既存のテストインフラ（Behavior テスト, Mailbox エミュレータ）を使ってシナリオ検証が可能。

## 5. TransportAdapter の責務
- フレームの送受信（バイト列管理）と、接続確立/切断イベントを BridgeActor へ通知する。
- 反対に BridgeActor からの送信要求を受けて書き込みを行う。
- no_std 向け（例: QUIC over UDP, UART）と std 向け（TCP/TLS）を差し替えられるよう trait 化。

## 6. 障害/復旧フロー
1. 接続エラーを TransportAdapter が検知 → BridgeActor へ `TransportFailure` を送信。  
2. BridgeActor は `SupervisorDirective` に従って再接続／バックオフを試行。  
3. 再接続中は EndpointRegistry へ `Pending` 状態を設定し、ユーザーメッセージは Scheduler 上の再送キューに積む。  
4. Quarantine 指定があった場合は EndpointRegistry が `Quarantine` に遷移させ、一定時間は接続試行を停止。  
5. ActorSystem シャットダウン時は `/system/remoting` 配下の Actor へ stop を送信し、TransportAdapter → Scheduler → TaskRunOnClose の順に安全に停止する。

## 7. 今後のタスク例
1. RemoteBridgeActor/TransportAdapter の skeleton 実装。  
2. Handshake/HeartBeat プロトコル定義（`Serialization` 使用）。  
3. EndpointRegistry + RemoteAuthorityManager の統合。  
4. Backpressure/Re-send キューと Metrics 公開。  
5. Typed/Classic 両 API からのリモート ActorRef 生成 (`RemoteActorRefProvider` 相当)。
