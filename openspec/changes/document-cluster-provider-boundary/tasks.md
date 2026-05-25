## 1. 境界ドキュメント

- [x] 1.1 local / static / AWS ECS provider の挙動を新しい provider boundary spec と照合する。
- [x] 1.2 `docs/plan/` に provider boundary の焦点を絞ったノートを追加する。
- [x] 1.3 cluster Grain runtime roadmap から boundary note へリンクする。

## 2. 契約カバレッジ

- [x] 2.1 既存の local provider tests が explicit join / leave / down の membership input を確認していることを確認する。
- [x] 2.2 既存の static provider tests が discovery なしの configured topology publication を確認していることを確認する。
- [x] 2.3 既存の std adapter tests が remoting subscription lifetime と weak provider retention を確認していることを確認する。
- [x] 2.4 既存の AWS ECS provider tests が startup / explicit down / unsupported join-leave / shutdown boundary を確認していることを確認する。

## 3. 検証

- [x] 3.1 `document-cluster-provider-boundary` の OpenSpec validation を実行する。
- [x] 3.2 `cluster-core` と `cluster-adaptor-std` の targeted cluster provider tests を実行する。
- [x] 3.3 変更した Markdown / Rust files の formatting checks を実行する。
