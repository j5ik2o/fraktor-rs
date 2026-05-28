## 1. Event Sourcing Effector rename

- [x] 1.1 `PersistenceEffector*` public files, types, re-exports, docs, and testsを`EventSourcedEffector*`へrenameする
- [x] 1.2 internal `PersistenceStoreActor` / command / reply / signal auth namesをevent-sourced semanticsへ揃える
- [x] 1.3 旧`PersistenceEffector*`名のpublic alias、deprecated item、compat moduleが残っていないことを`rg`で確認する
- [x] 1.4 rename後のevent-sourced flow testsとpublic-surface testsを更新して通す

## 2. State Sourcing public API

- [x] 2.1 `StateSourcedEffectorConfig`を追加し、`E`と`apply_event`を要求しないdurable state用configを定義する
- [x] 2.2 `StateSourcedEffectorSignal` / `StateSourcedEffectorSignalAuth` / `StateSourcedEffectorMessageAdapter`を追加する
- [x] 2.3 crate rootから`StateSourcedEffector*`をre-exportし、`DurableStateStore` contractはkernel側に残す
- [x] 2.4 external crateがtrusted state-sourced signalをforgeできないpublic-surface testを追加する

## 3. State Sourcing runtime

- [x] 3.1 durable state storeへ接続するinternal state-sourced store actor / command / replyを追加する
- [x] 3.2 起動時に`get_object(persistence_id)`で`Option<S>`とrevisionをrecoverし、`on_ready`へ渡す
- [x] 3.3 `persist_state`を実装し、`upsert_object`成功後にrevisionを進めてone-shot callbackが次の`Behavior<M>`を返すようにする
- [x] 3.4 `delete_state`を実装し、`delete_object`成功後にone-shot callbackが次の`Behavior<M>`を返すようにする
- [x] 3.5 recovery / persist / delete待機中のuser command stash / unstash semanticsをevent-sourced側と同等にする
- [x] 3.6 `DurableStateError`をstate-sourced effector failure signalへ変換し、default failureをfatal persistence failureとして扱う

## 4. Tests and documentation

- [x] 4.1 state-sourced recovery empty / present casesのfocused testsを追加する
- [x] 4.2 state-sourced persist success、delete success、revision mismatch failureのfocused testsを追加する
- [x] 4.3 `docs/gap-analysis/persistence-gap-analysis.md`を`EventSourcedEffector` / `StateSourcedEffector`前提へ更新する
- [x] 4.4 `openspec validate split-sourced-effector-apis --strict`を通す
- [x] 4.5 `cargo fmt --check --all`、targeted persistence-core-typed tests、`git diff --check`を通す
