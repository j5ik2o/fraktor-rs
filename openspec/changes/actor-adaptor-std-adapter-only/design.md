## Context

現在の `modules/actor-adaptor/src/std` には、`tracing` 連携のような runtime adapter と、Pekko classic logging に由来する public surface が同居している。後者は `std` 依存を持たず no_std でも成立する実装が多いにもかかわらず adapter 側へ置かれており、`std` 公開面を adapter-only に保つという既存方針と衝突している。

今回の change は、まず既存 classic logging family の所属を正すことに集中する。一方で fraktor-rs の `core` は no_std / Sans I/O を維持しなければならず、`tracing` のような backend 依存を持ち込むことはできない。`core` は `no_std + alloc` を前提とし、`alloc` 上で成立するデータ構造は利用できる。そのため「既存実装を `core` へ移し、`std` は adapter-only に縮める」ことを主眼にする。

## Goals / Non-Goals

**Goals:**
- 既存 classic logging family を `core::kernel::event::logging` の public surface として再配置する
- `std` 公開面を runtime adapter と std 固有 helper のみに縮小する

**Non-Goals:**
- Pekko の trait / mixin 形状を Rust へそのまま写経すること
- `core` へ `tracing` / SLF4J backend や std 依存を導入すること
- message-scoped MDC への再設計
- `LoggingReceive` の設定連動や log level gating の追加
- `LogEvent` timestamp 一元化の API 強化
- Pekko にある未実装型をこの change で新規追加すること
- typed logging 全体を今回の変更だけで Pekko typed の logger API へ完全一致させること
- runtime adapter 以外の unrelated std wrapper をこの change で一掃すること

## Decisions

### 1. classic logging family は `core` へ再配置する

現在コードベースに存在する `ActorLogMarker`、`ActorLogging`、`BusLogging`、`DiagnosticActorLogging`、`LoggingAdapter`、`LoggingReceive`、`NoLogging` は、`std` から削除するのではなく `core::kernel::event::logging` へ移す。これにより既存 capability を維持したまま、`std` を adapter-only にできる。

代替案:
- `std` から完全削除し `ActorContext` / `ActorSystem` の関数 API に吸収する
  - 利点: surface を減らせる
  - 欠点: 既存 classic logging family の public concept を消し、showcase / tests / 利用側の移行量が大きい
  - 却下。今回の目的は capability 削減ではなく責務の再配置だから。
- 現状のまま `std` に残す
  - 利点: 実装差分が最小
  - 欠点: `std` 境界が崩れ続け、core/std 分離方針に反する
  - 却下。adapter-only 方針と矛盾するため。

### 2. 今回は既存意味論を維持し、再設計は follow-up に分離する

`DiagnosticActorLogging` の message-scoped MDC、`LoggingReceive` の設定連動、timestamp 一元化の API 強化は、それぞれ再設計として独立の検討が必要である。この change では既存挙動を壊さずに型を `core` へ移すことを優先し、意味論の強化は follow-up change に分離する。

代替案:
- 移設と同時に message-scoped MDC、receive logging gating、timestamp 契約まで再設計する
  - 利点: Pekko 互換の意味論をまとめて前進できる
  - 欠点: 配置変更と意味論変更が混ざり、差分が大きくなる。既存 API 利用コードへの影響分析も必要になる
  - 却下。まず配置変更を片付け、その後に意味論変更を別 change として扱う方が安全。

### 3. `core` の logging は structured event emission のみを担う

`core` は no_std / Sans I/O を維持するため、logging capability は最終的に `LogEvent` を event stream へ publish する責務に限定する。実際の出力 backend への接続は `TracingLoggerSubscriber` など `std` 側 adapter が引き受ける。

代替案:
- `core` に backend 抽象を追加して logger sink を直接差し込む
  - 利点: 抽象的には柔軟
  - 欠点: 今回必要な以上の抽象化であり、Sans I/O の責務境界も曖昧にする
  - 却下。既存の event stream ベース設計を維持する。

### 4. `std` 側は adapter-only surface に縮小する

`modules/actor-adaptor/src/std/event/logging` は `TracingLoggerSubscriber` のみ、`std/event/stream` は `DeadLetterLogSubscriber` のみを残す。`subscriber_handle` や `EventStreamSubscriberShared` の shim、`std/pattern` の core 横流し helper は削除する。`std/pattern` は `StdClock`、`CircuitBreaker`、`CircuitBreakerShared`、`circuit_breaker`、`circuit_breaker_shared` を残存 API とする。

代替案:
- 互換性のため alias / re-export を残す
  - 利点: 利用側の差分が減る
  - 欠点: adapter-only 境界を再び曖昧にし、削除判断を先送りする
  - 却下。後方互換不要の開発フェーズであり、公開面を締める方を優先する。

## Risks / Trade-offs

- `core::kernel::event::logging` が actor context 依存を持ち込む → 既存 `LogEvent` / `LogLevel` と同じモジュールへ置くことで diff を小さく保つが、責務が広がりすぎる場合は follow-up で分割を検討する
- `std` shim 削除で showcase / tests の更新量が増える → `core` import path へ先に寄せてから削除する
- 既存型をそのまま移すだけでも import path の breaking change が生じる → 後方互換不要を前提に、一括更新で揃える
- 再設計を後送りするため Pekko 互換の意味論改善は残課題として残る → follow-up change で明示的に扱う

## Migration Plan

1. `core::kernel::event::logging` に既存 classic logging family を追加し、`std` 側実装を移設する
2. `std/system/*` の subscriber 型参照、showcase / tests / boundary test を `core` logging surface と `core` event stream 型へ移行する
3. 中間 re-export は置かず、単一 PR または短い系列で `std/event/logging`、`std/event/stream` shim、`std/pattern` wrapper を一括削除して adapter-only surface を固定する

## Open Questions

- `ActorLogMarker` を Pekko 語彙の `LogMarker` に寄せる rename を別 change として扱うか
- message-scoped MDC と `LoggingReceive` gating を別 change でどう切り出すか
- typed logging の logger-facing surface を Pekko typed にどこまで寄せるかは別 change で扱うか
