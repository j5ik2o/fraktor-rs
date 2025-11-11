# Actor-Core コア機能強化ロードマップ

## 概要

本ドキュメントは、pekkoおよびprotoactor-goと比較した際に、fraktor-rs/actor-coreで欠けているコア機能を特定し、実装優先度と推奨スケジュールを示すものです。

**方針**: Remoting、Cluster、Cluster Shardingなどの分散機能の前に、単一ノードでのアクターシステムのコア機能を充実させる。

## 機能比較の参照元

- **pekko**: `references/pekko/actor/`
- **protoactor-go**: `references/protoactor-go/actor/`, `references/protoactor-go/scheduler/`, `references/protoactor-go/router/`

---

## 🔴 優先度：最高（基本機能として必須）

### 1. Scheduler（システムスケジューラー）

**参照実装**:
- pekko: `actor/Scheduler.scala`
- protoactor-go: `scheduler/timer.go`

**欠けている機能**:
- `scheduleOnce`: 一度だけ遅延実行
- `scheduleWithFixedDelay`: 固定遅延での繰り返し実行（前回の実行完了から固定時間後）
- `scheduleAtFixedRate`: 固定レートでの繰り返し実行（前回の実行開始から固定時間後）
- システムレベルのタイマー管理
- `Cancellable`: キャンセル可能なタイマー参照
- スケジューリング精度の保証

**重要度**:
- タイムアウト、リトライ、定期処理の基礎インフラ
- 他の多くの機能（Timers、FSMなど）の前提条件

**実装規模**: 2-3週間

**技術的考慮事項**:
- `no_std`環境での時間管理（`embassy-time`などの検討）
- タイマーホイールアルゴリズムの実装（高スループット）
- メモリ効率的なキャンセル管理

---

### 2. Timers（アクター内タイマー）

**参照実装**:
- pekko: `actor/Timers.scala`

**欠けている機能**:
- アクター内での名前付きタイマー管理
- `startTimerWithFixedDelay(key, msg, delay)`: 固定遅延タイマー
- `startTimerAtFixedRate(key, msg, interval)`: 固定レートタイマー
- `startSingleTimer(key, msg, timeout)`: 単発タイマー
- `cancelTimer(key)`: 名前によるタイマーキャンセル
- `isTimerActive(key)`: タイマーの状態確認
- ライフサイクル連動（restart/stop時に自動キャンセル）

**重要度**:
- アクターパターンで頻繁に使用（タイムアウト処理、周期的な状態更新）
- 外部Schedulerへの直接依存を避け、アクターのカプセル化を維持

**実装規模**: 1-2週間（Schedulerに依存）

**使用例**:
```rust
impl<TB: RuntimeToolbox> Behavior<MyMessage, TB> for MyActor {
    fn on_message(&mut self, ctx: &mut impl ActorContext<TB>, msg: MyMessage) -> BehaviorDirective {
        match msg {
            MyMessage::Start => {
                ctx.timers().start_single_timer("timeout", MyMessage::Timeout, Duration::from_secs(5));
                BehaviorDirective::Same
            }
            MyMessage::Timeout => {
                // handle timeout
                BehaviorDirective::Same
            }
        }
    }
}
```

---

### 3. Stash（メッセージ保留）

**参照実装**:
- pekko: `actor/Stash.scala`

**欠けている機能**:
- `stash()`: 現在のメッセージを保留キューに追加
- `unstash()`: 保留キューから1つのメッセージを取り出してメールボックスの先頭に戻す
- `unstashAll()`: すべての保留メッセージをメールボックスに戻す
- `clearStash()`: 保留キューをクリア
- Deque-basedメールボックス連携（両端キュー）
- 容量制限とオーバーフロー戦略（Discard, Fail, Reply）
- `preRestart`/`postStop`時の自動`unstashAll()`

**重要度**:
- 状態遷移パターンで必須（例: 認証待ち、初期化待ち、リソース確保待ち）
- Behaviorの`become`と組み合わせて強力な状態管理を実現

**実装規模**: 2-3週間

**使用例**:
```rust
// 初期化中は他のメッセージを保留
impl<TB: RuntimeToolbox> Behavior<MyMessage, TB> for MyActor {
    fn on_message(&mut self, ctx: &mut impl ActorContext<TB>, msg: MyMessage) -> BehaviorDirective {
        match self.state {
            State::Initializing => {
                match msg {
                    MyMessage::InitComplete => {
                        self.state = State::Ready;
                        ctx.stash().unstash_all(); // 保留していたメッセージを処理
                        BehaviorDirective::Same
                    }
                    _ => {
                        ctx.stash().stash(); // 初期化完了まで保留
                        BehaviorDirective::Same
                    }
                }
            }
            State::Ready => {
                // 通常処理
                BehaviorDirective::Same
            }
        }
    }
}
```

---

### 4. Router（ルーター）

**参照実装**:
- protoactor-go: `router/router.go`, `router/roundrobin_router.go`, `router/broadcast_router.go`, `router/consistent_hash_router.go`

**欠けている機能**:
- **RoundRobin**: 順繰りルーティング
- **Broadcast**: 全routeeに配信
- **Random**: ランダム選択
- **ConsistentHash**: 一貫性ハッシュ（エンティティIDベース）
- **Pool**: 固定数のrouteeを自動管理
- **Group**: 既存のアクターパスからrouteeを動的管理
- Routeeライフサイクル管理（追加、削除、障害時の再生成）
- ルーティングロジックの抽象化（カスタムルーター実装可能）

**重要度**:
- 負荷分散の基礎インフラ
- 水平スケーリングパターンの実装
- 単一アクターのボトルネック回避

**実装規模**: 3-4週間

**使用例**:
```rust
// RoundRobinルーターでワーカーに負荷分散
let worker_props = Props::from_fn(|| WorkerActor);
let router = system.spawn_router(
    RouterConfig::round_robin(5), // 5つのワーカー
    worker_props,
    "worker-pool"
)?;

router.tell(WorkMessage::new(data)); // 自動的にワーカーにルーティング
```

---

## 🟡 優先度：高（実用機能として重要）

### 5. Middleware/Interceptor（ミドルウェアチェイン）

**参照実装**:
- protoactor-go: `actor/middleware_chain.go`, `actor/middleware/`

**欠けている機能**:
- **ReceiverMiddleware**: メッセージ受信時のインターセプト（前処理、後処理）
- **SenderMiddleware**: メッセージ送信時のインターセプト
- **ContextDecorator**: アクターコンテキストの拡張
- **SpawnMiddleware**: アクター生成時のフック
- ミドルウェアチェイン構築ロジック（順序制御）
- エラーハンドリングとフォールバック

**重要度**:
- 横断的関心事の統一実装（ロギング、メトリクス、トレーシング、認証）
- アクターコードとインフラコードの分離
- プラグイン機構の基盤

**実装規模**: 2-3週間

**使用例**:
```rust
// ロギングミドルウェアの例
struct LoggingMiddleware;

impl<TB: RuntimeToolbox> ReceiverMiddleware<TB> for LoggingMiddleware {
    fn intercept(&self, ctx: &mut impl ActorContext<TB>, msg: AnyMessage<TB>, next: ReceiverFunc<TB>) -> Result<(), ActorError> {
        log::debug!("Received message: {:?}", msg.type_name());
        let start = Instant::now();
        let result = next(ctx, msg);
        log::debug!("Processing took: {:?}", start.elapsed());
        result
    }
}

let props = Props::from_fn(|| MyActor)
    .with_receiver_middleware(LoggingMiddleware)
    .with_receiver_middleware(MetricsMiddleware);
```

---

### 6. Ask Pattern（Request-Response）

**参照実装**:
- protoactor-go: `actor/future.go`
- 既存: `modules/actor-core/src/futures.rs`（基本実装あり）

**欠けている機能**:
- `ask()`: 返信を期待するメッセージ送信の簡潔API
- タイムアウト付きFuture（`ErrTimeout`）
- 一時的な返信先アクター自動生成
- エラーハンドリング統一（`ErrDeadLetter`）
- `PipeTo`: Future結果を別アクターへ転送

**重要度**:
- 同期的なアクター呼び出しパターン
- 外部システムとの統合（REST API、データベース）
- クエリパターンの実装

**実装規模**: 1-2週間（既存Futureベース）

**使用例**:
```rust
// askパターン
let future = ctx.ask(database_actor, QueryMessage::new(user_id), Duration::from_secs(5))?;
match future.await {
    Ok(response) => { /* handle response */ }
    Err(AskError::Timeout) => { /* handle timeout */ }
    Err(AskError::DeadLetter) => { /* actor stopped */ }
}
```

---

### 7. FSM（有限状態機械）

**参照実装**:
- pekko: `actor/FSM.scala`

**欠けている機能**:
- 状態遷移DSL
- `when(state)`: 状態別メッセージハンドラ
- `goto(state)`: 状態遷移
- `stay()`: 現在の状態を維持
- `onTransition(from, to)`: 遷移時フック
- 状態タイムアウト（`StateTimeout`メッセージ）
- 遷移イベント通知（`SubscribeTransitionCallBack`）
- `Failure(cause)`による異常終了

**重要度**:
- 複雑な状態遷移ロジックの構造化
- プロトコル実装（認証フロー、注文処理など）
- 状態遷移の可視化とテスト容易性

**実装規模**: 4-5週間

**使用例**:
```rust
enum OrderState { Pending, Paid, Shipped, Delivered }
enum OrderData { Empty, OrderInfo(Order) }

impl<TB: RuntimeToolbox> FSM<OrderState, OrderData, TB> for OrderActor {
    fn when(&mut self, state: OrderState) -> StateHandler<TB> {
        match state {
            OrderState::Pending => {
                on_message(|msg, data| match msg {
                    OrderMessage::Pay => goto(OrderState::Paid).using(data),
                    OrderMessage::Cancel => stop(),
                })
                .with_timeout(Duration::from_secs(600))
            }
            OrderState::Paid => {
                on_message(|msg, data| match msg {
                    OrderMessage::Ship => goto(OrderState::Shipped).using(data),
                })
            }
            // ...
        }
    }

    fn on_transition(&mut self, from: OrderState, to: OrderState) {
        log::info!("Order state transition: {:?} -> {:?}", from, to);
    }
}
```

---

## 🟢 優先度：中（拡張機能として有用）

### 8. Throttler（流量制御）

**参照実装**:
- protoactor-go: `actor/throttler.go`

**欠けている機能**:
- メッセージレート制限（期間あたりの最大メッセージ数）
- バースト許容（短期的なスパイク対応）
- バックプレッシャー（送信側への通知）
- ウィンドウベース制御（スライディングウィンドウ）
- 超過時のコールバック（ロギング、アラート）

**重要度**:
- DoS対策
- リソース保護（CPU、メモリ、外部API）
- 公平性の保証（複数クライアント間）

**実装規模**: 1-2週間

**使用例**:
```rust
let throttler = NewThrottle::new(
    100,                         // 期間あたり最大100メッセージ
    Duration::from_secs(1),      // 1秒間
    |throttled_count| {
        log::warn!("Throttled {} messages", throttled_count);
    }
);

match throttler.should_throttle() {
    Valve::Open => { /* 処理続行 */ }
    Valve::Closing => { /* 警告 */ }
    Valve::Closed => { /* 拒否 */ }
}
```

---

### 9. Priority Queue（優先度付きメールボックス）

**参照実装**:
- protoactor-go: `actor/priority_queue.go`, `actor/unbounded_priority.go`

**欠けている機能**:
- メッセージ優先度設定（High, Normal, Low）
- 優先度ベースデキュー（高優先度を先に処理）
- システムメッセージの自動優先処理
- カスタム優先度関数（メッセージ内容ベース）
- 優先度別の統計情報

**重要度**:
- 緊急メッセージの優先処理（アラート、シャットダウン通知）
- QoSの実装
- システムメッセージ（Terminate、Restart）の確実な処理

**実装規模**: 1-2週間

---

### 10. CoordinatedShutdown（協調シャットダウン）

**参照実装**:
- pekko: `actor/CoordinatedShutdown.scala`

**欠けている機能**:
- フェーズ別シャットダウン（`before-service-unbind` → `service-unbind` → `service-stop` → ...）
- 依存関係順のクリーンアップ（親→子、サービス→コネクション）
- フェーズ別タイムアウト設定
- 外部システム連携フック（HTTPサーバー停止、DB接続クローズ）
- 失敗時のロギングとリカバリー

**重要度**:
- グレースフルシャットダウン（データロス防止）
- リソースリーク防止
- 運用時の安全な再起動

**実装規模**: 2-3週間

---

### 11. ActorSelection（パス選択）

**参照実装**:
- pekko: `actor/ActorSelection.scala`

**欠けている機能**:
- パスベースのアクター検索（`/user/parent/child`）
- ワイルドカード選択（`/user/*/child`, `/user/**/child`）
- ブロードキャスト送信（複数アクターへの一括送信）
- 動的な親子関係の探索
- `ActorIdentity`メッセージによる存在確認

**重要度**:
- 動的なアクター発見（現状はPIDベースのみ）
- 設定ベースのアクター参照
- テストでの柔軟なモック

**実装規模**: 2-3週間

---

## 📊 推奨実装順序（Phase別）

### Phase 1: 基本スケジューリング（4-6週間）
```
1. Scheduler (2-3週間)
   ├─ スケジューラーコア実装
   ├─ Cancellable実装
   └─ no_std対応

2. Timers (1-2週間) ※Schedulerに依存
   ├─ TimerScheduler trait
   ├─ アクターライフサイクル連動
   └─ 名前付きタイマー管理
```

**マイルストーン**: タイムアウト、定期実行の基本機能が使用可能

---

### Phase 2: メッセージ制御（5-7週間）
```
3. Stash (2-3週間)
   ├─ Deque-based mailbox
   ├─ stash/unstash API
   └─ オーバーフロー戦略

4. Router (3-4週間)
   ├─ ルーティング戦略（RoundRobin, Broadcast, Random, ConsistentHash）
   ├─ Pool/Group実装
   └─ Routeeライフサイクル

5. Ask Pattern (1-2週間)
   ├─ ask() API
   ├─ タイムアウト処理
   └─ PipeTo実装
```

**マイルストーン**: 実用的なメッセージパターンが揃う

---

### Phase 3: 拡張パターン（7-9週間）
```
6. Middleware (2-3週間)
   ├─ Receiver/Sender middleware
   ├─ チェイン構築
   └─ 標準ミドルウェア（Logging, Metrics）

7. FSM (4-5週間)
   ├─ 状態遷移DSL
   ├─ タイムアウト処理
   └─ 遷移イベント通知

8. Throttler (1-2週間)
   ├─ レート制限
   └─ バックプレッシャー
```

**マイルストーン**: 複雑な状態管理と横断的関心事に対応

---

### Phase 4: 運用機能（5-7週間）
```
9. Priority Queue (1-2週間)
   ├─ 優先度付きメールボックス
   └─ カスタム優先度関数

10. CoordinatedShutdown (2-3週間)
    ├─ フェーズ別シャットダウン
    └─ 依存関係管理

11. ActorSelection (2-3週間)
    ├─ パスベース検索
    └─ ワイルドカード対応
```

**マイルストーン**: 運用レベルの品質と安全性を実現

---

## 🎯 即座に着手すべきTOP3

### 1. Scheduler（最優先）
- **理由**:
  - すべての時間ベース機能の基礎インフラ
  - Timers、FSMの前提条件
  - 多くのアクターパターンで必要
- **インパクト**: 高
- **依存関係**: なし（独立実装可能）

### 2. Stash（第2優先）
- **理由**:
  - 状態遷移パターンで頻繁に必要
  - 既存Behaviorと組み合わせて強力
  - 実装が比較的独立
- **インパクト**: 高
- **依存関係**: Deque-based mailboxの拡張

### 3. Router（第3優先）
- **理由**:
  - 負荷分散、スケーラビリティの基本パターン
  - 単一アクターのボトルネック回避
  - 水平スケーリングの準備
- **インパクト**: 中〜高
- **依存関係**: なし（既存Props/Spawnベース）

---

## 実装時の技術的考慮事項

### no_std互換性
- すべての機能は`no_std`環境でも動作する必要がある
- 時間管理は`embassy-time`などの検討
- ヒープアロケーションは最小限に

### 既存コードとの統合
- 既存の`Behavior`、`ActorContext`、`Props`と自然に統合
- 破壊的変更を最小限に（必要なら歓迎）
- 型安全性の維持

### パフォーマンス
- ゼロコスト抽象化を目指す
- アロケーション最小化
- ロックフリーアルゴリズムの活用（可能な範囲で）

### テスト戦略
- 各機能に対する包括的なユニットテスト
- 統合テストによる相互作用の検証
- パフォーマンステスト（スループット、レイテンシ）

---

## 参考資料

- **Pekko Actor Documentation**: https://pekko.apache.org/docs/pekko/current/
- **ProtoActor Documentation**: https://proto.actor/docs/
- **Akka Classic Documentation**: https://doc.akka.io/docs/akka/current/

## 更新履歴

- 2025-11-10: 初版作成（pekko/protoactor-go比較分析）
