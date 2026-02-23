# タスク仕様

## 目的

streamsモジュールにmedium難易度のオペレーターと型（Phase 3）を実装し、Pekko Streamsの中級機能を追加する。

## 要件

- [ ] `debounce` オペレーターを実装する（最終要素から一定時間経過後に発行）
- [ ] `sample` オペレーターを実装する（一定間隔でサンプリング）
- [ ] `named` オペレーターを実装する（ステージ名の付与）
- [ ] `Source::from_materializer` / `Flow::from_materializer` を実装する
- [ ] `Sink::queue` を実装する（マテリアライズ値としてキューを提供）
- [ ] `Framing` ユーティリティを実装する（デリミタベースのフレーミング）
- [ ] `FlowWithContext` / `SourceWithContext` を実装する（コンテキスト伝搬付きストリーム）
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- 時間ベースオペレーター（debounce, sample）がTickDriverと統合されている
- FlowWithContext/SourceWithContextが既存のFlow/Source APIと一貫した形で提供される
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/streams-gap-analysis.md`
- Pekko参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/`
- Pekko参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/FlowWithContext.scala`
