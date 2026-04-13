## 1. TerminationSignal の導入

- [x] 1.1 `TerminationState` と `TerminationSignal` を追加し、termination 専用の non-consuming state を定義する
- [x] 1.2 `SystemState` の terminated 状態を `TerminationState` の単一 source of truth へ統合する
- [x] 1.3 core に `Blocker` port 契約を追加し、`TerminationSignal` から利用できるようにする
- [x] 1.4 `TerminationSignal` に async `IntoFuture` 契約を追加する

## 2. actor system termination API の置換

- [x] 2.1 `ActorSystem::when_terminated()` を `TerminationSignal` 返却へ変更する
- [x] 2.2 `TypedActorSystem::when_terminated()` と `get_when_terminated()` を `TerminationSignal` 返却へ変更する
- [x] 2.3 std adapter に `Blocker` 実装を追加する
- [x] 2.4 `run_until_terminated()` の扱いを見直し、`Blocker` 契約経由へ揃えるか adapter 側 helper へ移すかを決定して反映する

## 3. sample / test / spec 追随

- [x] 3.1 `showcases/std/getting_started/main.rs` と関連 sample を `TerminationSignal` と std adapter の `Blocker` 実装を使う形へ更新する
- [x] 3.2 classic / typed の termination tests を `TerminationSignal` 契約に合わせて更新する
- [x] 3.3 `when_terminated()` 利用箇所で `ActorFutureShared<()>` へ直接依存している public 経路が残っていないことを確認する
