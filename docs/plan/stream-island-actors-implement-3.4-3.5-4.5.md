# stream-island-actors 実装計画 3.4 / 3.5 / 4.5

## 対象

- 3.4: 未登録 dispatcher 指定時に materialization が失敗し、default dispatcher へフォールバックしないことを固定する。
- 3.5: island actor spawn 途中で失敗した場合、起動済み actor / tick resource / boundary resource を rollback する。
- 4.5: shutdown failure を `StreamError` または actor error として観測できるようにし、戻り値を黙殺しない。

## 方針

- `ActorMaterializer` の既存 rollback / shutdown 経路に沿って、戻り値を握りつぶさず first error として観測可能にする。
- 未登録 dispatcher は default dispatcher に暗黙 fallback させない。
- materialization 途中で失敗した stream resource は materializer 内部に登録せず、起動済み actor / scheduler job / stream を cleanup する。
- 先行テストは既存の `actor_materializer/tests.rs` に合わせ、必要な場合のみ補正する。
