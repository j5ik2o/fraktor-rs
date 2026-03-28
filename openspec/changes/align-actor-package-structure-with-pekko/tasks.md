## 1. 変更土台の確立

- [ ] 1.1 proposal / spec / design を確定し、breaking change と package 再編方針を固定する
- [ ] 1.2 `modules/actor/src/core` の現行 package と公開 import path を棚卸しし、`kernel` と `typed` の仕分け表を作る
- [ ] 1.3 実装開始時の運用として、file edit ごとに `./scripts/ci-check.sh ai dylint` を実行する手順を作業順へ組み込む

## 2. core 最上位軸の再編

- [ ] 2.1 `modules/actor/src/core.rs` を `kernel` / `typed` 軸に再編する
- [ ] 2.2 既存の untyped runtime package を `core/kernel/` 側へ移設し、`mod` 配線を修正する
- [ ] 2.3 `core/kernel` 化に追随して import path と tests を更新し、各編集後に `./scripts/ci-check.sh ai dylint` を実行する

## 3. typed/receptionist の再編

- [ ] 3.1 `core/typed/receptionist/` を新設し、`Receptionist`、`ReceptionistCommand`、`ServiceKey`、`Listing` を移設する
- [ ] 3.2 receptionist 依存の import path と module path を更新する
- [ ] 3.3 各 file edit 後に `./scripts/ci-check.sh ai dylint` を実行して module wiring を確認する

## 4. typed/pubsub の再編

- [ ] 4.1 `core/typed/pubsub/` を新設し、`Topic`、`TopicCommand`、`TopicStats` を移設する
- [ ] 4.2 pubsub から receptionist を参照する path を新構造へ合わせて更新する
- [ ] 4.3 各 file edit 後に `./scripts/ci-check.sh ai dylint` を実行する

## 5. typed/routing の再編

- [ ] 5.1 `core/typed/routing/` を新設し、`Routers`、`Resizer`、各 router builder を移設する
- [ ] 5.2 routing から receptionist / typed primitive を参照する path を新構造へ合わせて更新する
- [ ] 5.3 各 file edit 後に `./scripts/ci-check.sh ai dylint` を実行する

## 6. typed root と std 追随

- [ ] 6.1 `core/typed.rs` の公開面を typed primitive に絞り、receptionist / pubsub / routing は package 経由参照へ切り替える
- [ ] 6.2 `modules/actor/src/std`、tests、examples の import path と package 参照を新構造へ追随させる
- [ ] 6.3 最終確認として `./scripts/ci-check.sh ai all` を実行し、完了条件を満たすことを確認する
