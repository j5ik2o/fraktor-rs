# Gap Analysis: scheduler-tick-driver-integration

## 1. 現状把握
- **Scheduler/Tick 供給**: `RuntimeToolbox::tick_source` は単に `SchedulerTickHandle::scoped` を返すだけであり、実時間を監視したりドリフト監視を行う driver は存在しない。`StdToolbox` も `ManualClock` を前提にしているため、Tokio などホスト環境で自動 tick を生成する経路がない。`
- **ActorSystem 統合**: `ActorSystem::ensure_scheduler_context` は `SchedulerContext::new(TB::default(), SchedulerConfig::default())` を強制し、構成 API や driver 注入ポイントを提供していない。`SchedulerRunner` は `pub` のまま manual モード専用実装で、ユーザコードが Runner API に直接依存している。
- **Observability/Docs**: EventStream のイベント種別に scheduler メトリクスがなく、tick 数の公開や driver 停止イベントの通知手段がない。Quickstart／ガイド類は dispatcher や guardian を扱うのみで、TickDriverConfig・環境別テンプレート・トラブルシュートを記載していない。
- **参照実装との差**: protoactor-go の `TimerScheduler` は `time.AfterFunc` でバックグラウンドタイマを管理し、キャンセルや繰り返し呼び出しを隠蔽しているが、fraktor 側には同等の driver 抽象が存在しない。

## 2. 要件適合性マッピング
| 要件 | 既存資産 | ギャップ種別 | メモ |
| --- | --- | --- | --- |
| R1 std 自動 tick 供給 | `StdToolbox`、`SchedulerRunner`、`EventStream` | Missing | 自動 driver・Tokio タスク生成・ドリフト±5% 監視・メトリクス発行が未実装。ホストタイマ登録失敗時のフェイルファストも不在。 |
| R2 no_std ドライバ抽象化 | `SchedulerTickHandle`、`TickState`、`SchedulerContext` | Missing / Constraint | 外部 driver trait や attach API がなく、tick 順序保証・停止イベント通知も未定義。`TickState` が `swap(0)` のみで FIFO を維持できない。 |
| R3 Runner API テスト限定 | `SchedulerRunner` が `pub use` され examples でも使用 | Missing | プロダクション構成で Runner API を拒否する仕組みが無い。モードの記録や構成エラー経路も未実装。 |
| R4 Quickstart/Driver ガイド | `docs/guides/actor-system.md`、`specs/001-add-actor-runtime/quickstart.md` | Missing | TickDriverConfig や driver マトリクス、main テンプレート、トラブルシュート等の情報が空白。 |

## 3. 実装アプローチ候補
### Option A: 既存コンポーネント拡張
- `RuntimeToolbox` に driver trait を追加し、`StdToolbox` が Tokio interval を spawn、`NoStdToolbox` が外部 driver 実装を受け付ける。`SchedulerContext` が driver lifecycle を握り、EventStream へ tick metrics を publish。
- **Pros**: 既存依存関係に沿って実装しやすく、actor-core だけで完結。
- **Cons**: Toolbox 変更が全クレートへ波及し、std/no_std/typed すべての API 互換性が崩れる。tokio タスク管理を core で扱うため責務が肥大化。

### Option B: 新規 Driver Manager / Bootstrap 層
- `TickDriverConfig` と `ActorSystemBootstrap` を新設し、`ActorSystem` 起動前に driver を選択・注入。`SchedulerContext` は既存のまま、別コンポーネントが `SchedulerRunner` をバックグラウンドで駆動。
- **Pros**: core の破壊的変更を抑えつつ利用者 UX を改善。std/no_std で driver 実装を分離しやすい。
- **Cons**: Builder → Core への受け渡し経路を増やす必要があり、初期化順序や shutdown hooks の設計が複雑。

### Option C: ハイブリッド
- Option A で driver hook を最小限導入しつつ、並行して Option B の bootstrap API を整備。短期で R1/R2 を満たしつつ長期的に UX を向上。
- **Pros**: driver trait を早期確立でき、docs も builder 視点で更新可能。
- **Cons**: 過渡期に二重の初期化経路が並存し、明確な deprecation ポリシーが必須。

## 4. 工数・リスク評価
- **Effort**: L（1〜2週間）。RuntimeToolbox、Scheduler、ActorSystem、actor-std、docs まで波及し、std/no_std テストおよび Quickstart の全面更新が必要。
- **Risk**: High。バックグラウンド driver が scheduler の determinism と安全性を左右し、Tokio/embassy 双方でのジッタ管理や停止シーケンス失敗が ActorSystem 全体の安定性を損なう可能性が高い。

## 5. Research Needed
1. Tokio interval/`sleep` ベース driver のジッタと ±5% ドリフト要件をどう満たすか（run-time affinity、専用スレッド化の要否）。
2. embassy/SysTick など割り込み駆動 driver が `SchedulerTickHandle` を安全に操作できるか、もしくは lock-free queue を別途用意する必要があるか。
3. `TickState` の `u32` カウンタを順序保存型へ拡張する手段と、マルチソースからの tick フュージョン手法。
4. ActorSystem shutdown で driver を確実に停止し、`SystemState::shutdown_scheduler` と整合させる設計。
5. EventStream へ追加する Scheduler メトリクス（tick/s、driver 状態）と LoggerSubscriber 連携のフォーマット。
6. Quickstart で提示する builder API（`ActorSystemBootstrap` など）の UX と、既存 `ActorSystem::new_*` との共存戦略。
