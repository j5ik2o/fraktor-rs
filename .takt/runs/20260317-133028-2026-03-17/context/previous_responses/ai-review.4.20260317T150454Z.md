前回までの残件が解消した前提で、`ai-review.md` の open findings を起点にゼロベースで再確認します。今回もレポート履歴は参照しつつ、最終判断は現物コードだけで下します。
前回の残件は `check-unit-sleep` の glob と `circuit_breaker_shared` 側の sleep/comment でした。現物にまだ残っていないかを検索して、再オープン条件に当たるものだけ拾います。
前回の2件は現物では解消に見えます。最後に、変更ファイル内の未使用コードや説明コメント、公開 API の再流出がないかだけ横断確認します。
**Findings**
- `finding_id: ai-review-f001` `status: resolved`  
  前回の `sleep` 検査問題は解消しています。`check_unit_sleep` は [scripts/ci-check.sh:1017](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/scripts/ci-check.sh#L1017) で `rg` を使い、[scripts/ci-check.sh:1004](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/scripts/ci-check.sh#L1004) と [scripts/ci-check.sh:1005](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/scripts/ci-check.sh#L1005) は `**/tests.rs` / `**/tests/*.rs` に修正済みです。前回未解決だった [modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs:164](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs#L164) の `tokio::time::sleep` も、現在は [modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs:163](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs#L163) の `std::future::pending` に置き換わっています。  
  修正不要です。

- `finding_id: ai-review-f002` `status: resolved`  
  `new_with_clock` の公開範囲は引き続き [modules/actor/src/std/pattern/circuit_breaker.rs:77](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker.rs#L77) と [modules/actor/src/std/pattern/circuit_breaker_shared.rs:42](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared.rs#L42) の両方で `pub(crate)` を維持しており、public API 漏洩は再発していません。  
  修正不要です。

- `finding_id: ai-review-f003` `status: resolved`  
  前回指摘した説明コメントは解消しています。`RAII ガード` / `正常完了` コメントは [modules/actor/src/std/pattern/circuit_breaker_shared.rs:72](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared.rs#L72) 付近から消えており、`scheduler/tick/tests.rs` の `Given/When/Then` も残っていません。`quickstart.rs` からも待機説明コメントは除去済みで、現在残っているのはドメイン上の補足や rustdoc だけです。  
  修正不要です。

**判定**
`APPROVE`

ビルド系コマンドはこのムーブメントの制約で実行していませんが、静的確認では `new` / `persists` のブロッキング問題は見つかりませんでした。