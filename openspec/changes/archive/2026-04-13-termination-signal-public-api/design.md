## Context

現在の actor system termination API は `ActorFutureShared<()>` を直接返しており、利用者は `with_read` / `with_write` を通じて内部 future primitive を直接操作できる。これは ask 用の内部共有 future と termination 観測の public 契約を同一型にしている状態であり、同期 `main` では busy wait、非同期 `main` では `ActorFutureListener` の知識が必要になるなど、利用者にランタイム内部事情を漏らしている。

さらに、既存 `ActorFutureShared<()>` は `try_take()` による単一消費 semantics を前提としているため、複数 observer が非消費的に終了状態を観測する termination 契約とは整合しない。termination 観測の公開面は ask future とは別の意味論を持つ専用状態へ分離する必要がある。

この change では ask 系の整理には踏み込まず、termination 観測だけを先に切り出す。最小変更で public 契約を改善しつつ、termination 専用状態を新設して `when_terminated()` 系 API だけを差し替える。

## Goals / Non-Goals

**Goals:**
- `when_terminated()` / `get_when_terminated()` の戻り値を termination 専用公開型へ変更する
- 非同期コードからは内部 listener 型を直接扱わずに await できる public API を与える
- `when_terminated()` 系 API に限って `ActorFutureShared<()>` を内部詳細へ押し戻し、termination 観測と ask future の意味論を public 面で分離する
- std に依存する blocking wait を core へ持ち込まない
- 同期 blocking wait は core の `Blocker` port と std adapter 実装の組み合わせで実現する

**Non-Goals:**
- `ActorFuture` / `ActorFutureShared` 全体の public/private 見直し
- ask 系 public API (`AskResponse`, `TypedAskFuture`, `drain_ready_ask_futures`) の redesign
- core に同期 blocking wait API を追加すること

## Decisions

### 1. `when_terminated()` は `TerminationSignal` を返し、内部状態は termination 専用に分離する

`ActorSystem::when_terminated()`、`TypedActorSystem::when_terminated()`、`TypedActorSystem::get_when_terminated()` は `ActorFutureShared<()>` ではなく `TerminationSignal` を返す。`TerminationSignal` は `ActorFutureShared<()>` を内部利用せず、termination 専用の `TerminationState` を参照する。`TerminationState` は non-consuming な完了状態と通知機構を持つ。

代替案:
- `ActorFutureShared<()>` を public のまま改善する
  - 却下理由: ask 系と termination の意味論が衝突し、単一消費 future と non-consuming 終了観測を 1 型で両立させにくい
- `ActorFutureShared<()>` を内部に保持した thin wrapper にする
  - 却下理由: `try_take()` と `is_ready()` の単一消費 semantics が残るため、複数 observer が同じ終了状態を観測する termination 契約に反する
- `when_terminated()` へ新しい helper を追加するだけに留める
  - 却下理由: 低レベル primitive が public に残り、誤用経路を閉じられない

### 2. `TerminationState` を `SystemState` の唯一の終了状態 source of truth にする

`SystemState` が持つ終了状態は `TerminationState` に統合する。`SystemState::mark_terminated()` は `TerminationState` を完了させ、`SystemState::is_terminated()` はその state を読む。終了状態の真実源を `AtomicBool` と別 future に二重化してはならない。

代替案:
- 既存 `SystemState::terminated: AtomicBool` を維持し、別途 `TerminationSignal` 側にも状態を持つ
  - 却下理由: 終了状態の二重化により更新漏れ・観測ずれを招きやすい

### 3. `TerminationSignal` は non-consuming な観測契約を持つ

termination は複数の観測者が同時に扱えるべきなので、`TerminationSignal` は clone 可能で、どれか 1 つが待機または観測しても他の clone の可視状態を消費しない。`is_terminated()` は単調に false → true のみを許す。

代替案:
- `try_take()` 型の単一消費 API を流用する
  - 却下理由: termination は broadcast 的な終端状態であり、ask reply と同じ消費 semantics を持たせると観測者間で競合する

### 4. 同期 blocking wait は `Blocker` port と std adapter 実装で実現する

`TerminationSignal` は `core` に配置し、常に `is_terminated()` と async `IntoFuture` を提供する。同期 blocking wait が必要な場合は、core に `Blocker` port 契約を定義し、std adapter が `Condvar` ベースの実装を提供する。`TerminationSignal` は `Blocker` 契約を受け取って同期待機できるが、std 実装型そのものは知らない。

代替案:
- `wait_blocking()` を core 型へ `#[cfg(feature = "std")]` で追加する
  - 却下理由: このプロジェクトでは `std` は adapter モジュールでしか使わず、core に platform-specific 依存を混ぜない
- core に port を置かず、std adapter 側 helper だけで termination を待つ
  - 却下理由: termination の同期待機契約が public API として分散し、利用者から見た一貫性を欠く
- async 側だけ整備して同期側は `run_until_terminated()` の busy wait を残す
  - 却下理由: sample / public 契約として busy wait を温存する設計になり、今回の動機を十分に解消できない

### 5. no_std / std の分離は `cfg(feature = "std")` で閉じる

termination 観測の共通契約は `core` に置き、platform-specific な補助は `std` adapter 側に隔離する。core は `std::sync::Condvar` を含む std 依存を持たない。一方で、同期待機契約そのものは `Blocker` port として core に置き、実装だけを std へ逃がす。

代替案:
- `TerminationSignal` 自体を `std` 側へ置く
  - 却下理由: async/no_std から termination を観測する共通 public 型を失う
- core 内部で `cfg(feature = "std")` によって `Condvar` 実装を抱える
  - 却下理由: モジュール責務として core / std 分離を破る

### 6. sample / doc は `TerminationSignal` の推奨経路へ寄せる

`showcases/std/getting_started/main.rs` を含むサンプルは、新しい `TerminationSignal` API を使うように変更する。同期サンプルでは blocking wait、非同期サンプルでは await を優先し、`thread::yield_now()` ループを推奨しない。

## Risks / Trade-offs

- [既存利用者の破壊的変更] → `when_terminated()` の戻り値変更を proposal/spec/tasks で明示し、ask 系は今回触らないことで影響範囲を termination API に限定する
- [同期サンプルの待機経路が 2 段になる] → `TerminationSignal` は core の `Blocker` 契約だけを知り、std adapter 側にデフォルト実装を用意して利用者の手数を抑える
- [`TerminationSignal` だけ先行すると public future 設計が一時的に不統一になる] → non-goal として明記し、次段階で ask 系 public surface を整理可能な状態に留める

## Migration Plan

1. `TerminationState` と `TerminationSignal` を追加し、`SystemState` の終了状態 source of truth を移す
2. core に `Blocker` port 契約を追加し、`TerminationSignal` から利用できるようにする
3. `TerminationSignal` に `IntoFuture` を実装する
4. std adapter に `Blocker` 実装を追加する
5. classic / typed の `when_terminated()` 系 API を新型へ切り替える
6. sample / test を `TerminationSignal` + `Blocker` 契約へ更新し、busy wait を残す箇所を明示的に減らす

## Open Questions

- std adapter 側の `Blocker` 実装を `std::dispatch` に置くか、`std::system` に置くか
