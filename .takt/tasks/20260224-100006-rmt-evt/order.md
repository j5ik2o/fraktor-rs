# タスク仕様

## 目的

remoteモジュールにイベント型、制御メッセージ、信頼配信メカニズム（Phase 1-3: trivial〜medium）を実装し、Pekkoリモーティングのライフサイクル管理と信頼性を強化する。

## 要件

- [ ] `GracefulShutdownQuarantinedEvent` イベント型を追加する
- [ ] `ThisActorSystemQuarantinedEvent` イベント型を追加する
- [ ] `RemotingLifecycleEvent` enumを実装する（型付きイベントの体系化）
- [ ] `ControlMessage` traitを実装する（制御メッセージの型階層）
- [ ] `RemoteInstrument` traitを実装する（カスタム計装フック）
- [ ] `TestTransport` を実装する（テスト用モックトランスポート）
- [ ] `SystemMessageEnvelope` / `AckedDelivery` を実装する（システムメッセージの信頼配信）
- [ ] `RemoteWatcher` にハートビートプロトコル（`Heartbeat` / `HeartbeatRsp`）を実装する
- [ ] `Flush` / `FlushAck` 制御メッセージを実装する（グレースフルシャットダウン保証）
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- ライフサイクルイベントが型付きenumとして体系化されている
- システムメッセージが確認応答付きで信頼配信される
- RemoteWatcherがハートビートを送受信し、FDと連携する
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/remote-gap-analysis.md`（カテゴリ5, 6, 8, 10）
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/RemotingLifecycleEvent.scala`
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/SystemMessageDelivery.scala`
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/RemoteWatcher.scala`
