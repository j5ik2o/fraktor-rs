# Pekko Streams 未対応オペレーター一覧

## 概要
- 作成日: 2026-02-10
- 比較対象（Pekko）: `references/pekko/docs/src/main/paradox/stream/operators/index.md`
- 比較対象（fraktor-rs）:
  - `modules/streams/src/core/source.rs`
  - `modules/streams/src/core/flow.rs`
  - `modules/streams/src/core/sink.rs`
  - `modules/streams/src/core/operator_key.rs`
- 比較方法: Pekko の operator index にあるアンカー名（`<a name="...">`）と、`modules/streams` の公開メソッド/`OperatorKey` を正規化して照合

## 集計
- Pekko 側アンカー総数: `210`
- 一致（実装済み推定）: `41`
- 未対応: `169`

## 注意
- 本一覧は index アンカー名ベースの差分であり、名称差分がある実装（例: restart/supervision/hub/kill switch の粒度差）は別途手動確認が必要。
- 例:
  - `modules/streams/src/core/source.rs:231`
  - `modules/streams/src/core/flow.rs:171`
  - `modules/streams/src/core/sink.rs:104`
  - `modules/streams/src/core/merge_hub.rs:11`

## 実装済み一覧

### 実装済み（41）
- [x] `balance`
- [x] `broadcast`
- [x] `buffer`
- [x] `concat`
- [x] `drop`
- [x] `dropwhile`
- [x] `empty`
- [x] `filter`
- [x] `filternot`
- [x] `flattenoptional`
- [x] `flatmapconcat`
- [x] `flatmapmerge`
- [x] `fold`
- [x] `fromarray`
- [x] `fromiterator`
- [x] `fromoption`
- [x] `foreach`
- [x] `groupby`
- [x] `grouped`
- [x] `head`
- [x] `ignore`
- [x] `intersperse`
- [x] `last`
- [x] `map`
- [x] `mapconcat`
- [x] `mapoption`
- [x] `merge`
- [x] `recover`
- [x] `recoverwithretries`
- [x] `scan`
- [x] `single`
- [x] `sliding`
- [x] `splitafter`
- [x] `splitwhen`
- [x] `statefulmap`
- [x] `statefulmapconcat`
- [x] `take`
- [x] `takeuntil`
- [x] `takewhile`
- [x] `zip`
- [x] `zipwithindex`

## 未対応一覧

### Source operators (31)
- [ ] `assourcewithcontext`
- [ ] `assubscriber`
- [ ] `combine`
- [ ] `completionstage`
- [ ] `completionstagesource`
- [ ] `cycle`
- [ ] `failed`
- [ ] `from`
- [ ] `fromjavastream`
- [ ] `frompublisher`
- [ ] `future`
- [ ] `futuresource`
- [ ] `iterate`
- [ ] `lazycompletionstage`
- [ ] `lazycompletionstagesource`
- [ ] `lazyfuture`
- [ ] `lazyfuturesource`
- [ ] `lazysingle`
- [ ] `lazysource`
- [ ] `maybe`
- [ ] `never`
- [ ] `queue`
- [ ] `range`
- [ ] `repeat`
- [ ] `tick`
- [ ] `unfold`
- [ ] `unfoldasync`
- [ ] `unfoldresource`
- [ ] `unfoldresourceasync`
- [ ] `zipn`
- [ ] `zipwithn`

### Sink operators (25)
- [ ] `aspublisher`
- [ ] `cancelled`
- [ ] `collect`
- [ ] `collection`
- [ ] `completionstagesink`
- [ ] `count`
- [ ] `exists`
- [ ] `foldwhile`
- [ ] `forall`
- [ ] `foreachasync`
- [ ] `frommaterializer`
- [ ] `fromsubscriber`
- [ ] `futuresink`
- [ ] `headoption`
- [ ] `lastoption`
- [ ] `lazycompletionstagesink`
- [ ] `lazyfuturesink`
- [ ] `lazysink`
- [ ] `none`
- [ ] `oncomplete`
- [ ] `prematerialize`
- [ ] `reduce`
- [ ] `seq`
- [ ] `source`
- [ ] `takelast`

### Additional Sink and Source converters (7)
- [ ] `asinputstream`
- [ ] `asjavastream`
- [ ] `asoutputstream`
- [ ] `frominputstream`
- [ ] `fromoutputstream`
- [ ] `javacollector`
- [ ] `javacollectorparallelunordered`

### File IO Sinks and Sources (2)
- [ ] `frompath`
- [ ] `topath`

### Simple operators (30)
- [ ] `asflowwithcontext`
- [ ] `collectfirst`
- [ ] `collecttype`
- [ ] `collectwhile`
- [ ] `completionstageflow`
- [ ] `contramap`
- [ ] `detach`
- [ ] `dimap`
- [ ] `dooncancel`
- [ ] `doonfirst`
- [ ] `droprepeated`
- [ ] `foldasync`
- [ ] `futureflow`
- [ ] `groupedadjacentby`
- [ ] `groupedadjacentbyweighted`
- [ ] `groupedweighted`
- [ ] `lazycompletionstageflow`
- [ ] `lazyflow`
- [ ] `lazyfutureflow`
- [ ] `limit`
- [ ] `limitweighted`
- [ ] `log`
- [ ] `logwithmarker`
- [ ] `mapwithresource`
- [ ] `materializeintosource`
- [ ] `optionalvia`
- [ ] `scanasync`
- [ ] `throttle`

### Flow operators composed of Sinks and Sources (2)
- [ ] `fromsinkandsource`
- [ ] `fromsinkandsourcecoupled`

### Asynchronous operators (4)
- [ ] `mapasync`
- [ ] `mapasyncpartitioned`
- [ ] `mapasyncpartitionedunordered`
- [ ] `mapasyncunordered`

### Timer driven operators (7)
- [ ] `delay`
- [ ] `delaywith`
- [ ] `dropwithin`
- [ ] `groupedweightedwithin`
- [ ] `groupedwithin`
- [ ] `initialdelay`
- [ ] `takewithin`

### Backpressure aware operators (7)
- [ ] `aggregatewithboundary`
- [ ] `batch`
- [ ] `batchweighted`
- [ ] `conflate`
- [ ] `conflatewithseed`
- [ ] `expand`
- [ ] `extrapolate`

### Nesting and flattening operators (4)
- [ ] `flatmapprefix`
- [ ] `flattenmerge`
- [ ] `prefixandtail`
- [ ] `switchmap`

### Time aware operators (5)
- [ ] `backpressuretimeout`
- [ ] `completiontimeout`
- [ ] `idletimeout`
- [ ] `initialtimeout`
- [ ] `keepalive`

### Fan-in operators (18)
- [ ] `mergesequence`
- [ ] `concatalllazy`
- [ ] `concatlazy`
- [ ] `interleave`
- [ ] `interleaveall`
- [ ] `mergeall`
- [ ] `mergelatest`
- [ ] `mergepreferred`
- [ ] `mergeprioritized`
- [ ] `mergeprioritizedn`
- [ ] `mergesorted`
- [ ] `orelse`
- [ ] `prepend`
- [ ] `prependlazy`
- [ ] `zipall`
- [ ] `ziplatest`
- [ ] `ziplatestwith`
- [ ] `zipwith`

### Fan-out operators (7)
- [ ] `partition`
- [ ] `unzip`
- [ ] `unzipwith`
- [ ] `alsoto`
- [ ] `alsotoall`
- [ ] `divertto`
- [ ] `wiretap`

### Watching status operators (2)
- [ ] `monitor`
- [ ] `watchtermination`

### Actor interop operators (8)
- [ ] `actorref`
- [ ] `actorrefwithbackpressure`
- [ ] `ask`
- [ ] `askwithcontext`
- [ ] `askwithstatus`
- [ ] `askwithstatusandcontext`
- [ ] `sink`
- [ ] `watch`

### Compression operators (4)
- [ ] `deflate`
- [ ] `gzip`
- [ ] `gzipdecompress`
- [ ] `inflate`

### Error handling (8)
- [ ] `maperror`
- [ ] `onerrorcomplete`
- [ ] `onerrorcontinue`
- [ ] `onerrorresume`
- [ ] `onfailureswithbackoff`
- [ ] `recoverwith`
- [ ] `withbackoff`
- [ ] `withbackoffandcontext`
