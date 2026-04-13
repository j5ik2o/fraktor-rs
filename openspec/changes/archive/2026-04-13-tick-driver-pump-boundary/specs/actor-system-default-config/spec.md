## MODIFIED Requirements

### Requirement: ActorSystem::new() は責務再編後もデフォルト tick driver 構成を提供する

`ActorSystem::new(&props)` は、tick driver 配線の責務を `core` へ移した後も、`tokio-executor` feature 有効時にデフォルト tick driver 構成で起動できなければならない。利用者は内部 wiring の再設計を意識せず、従来どおりデフォルト起動できることを MUST 満たす。

#### Scenario: tokio-executor 有効時に new() がデフォルトで動作する
- **WHEN** `#[cfg(feature = "tokio-executor")]` が有効な環境で `ActorSystem::new(&props)` を呼び出す
- **THEN** 10ms resolution のデフォルト TickDriver と現在の Tokio runtime handle を使った Dispatcher でシステムが起動する
- **AND** tick driver の内部配線再編を利用者が意識する必要はない
