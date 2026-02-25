# タスク仕様

## 目的

actorモジュールにタイマー、プールルーター、ビヘイビアインターセプタ（Phase 3: medium）を実装し、Pekko Typed Actorの主要なユーティリティ機能を追加する。

## 要件

- [ ] `TimerScheduler<M>` trait/structを実装する（`startTimerWithFixedDelay`, `startTimerAtFixedRate`, `startSingleTimer`, `cancel`）
- [ ] `Behaviors::with_timers` ファクトリを実装する（TickDriverとの統合）
- [ ] `ActorContext::set_receive_timeout` を実装する（アイドルタイムアウト）
- [ ] `Routers::pool(size, behavior)` を実装する（固定サイズプールルーター、ラウンドロビン）
- [ ] `BehaviorInterceptor` traitを実装する（ビヘイビアの横断的関心事インターセプト）
- [ ] `Behaviors::intercept` ファクトリを実装する
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- TimerSchedulerがTickDriverと統合され、tickベースの実行モデルで動作する
- プールルーターが子アクター生成とラウンドロビン分配を行う
- BehaviorInterceptorでメッセージ/シグナルの前後処理が可能
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/actor-gap-analysis.md`（カテゴリ2, 3, 6, 7）
- Pekko参照: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/TimerScheduler.scala`
- Pekko参照: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala`
- Pekko参照: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/BehaviorInterceptor.scala`
