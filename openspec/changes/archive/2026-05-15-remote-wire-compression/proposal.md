## 背景

remote のペイロードシリアライズ、reliable DeathWatch、graceful flush が wire lane に接続されたため、残る compression gap は actor ref と serializer manifest の compression table を実際の fraktor-native wire path へ適用する段階に入っている。

現状の `RemoteCompressionConfig` は設定 surface として保持されるだけで、table advertisement、acknowledgement、hit counting、compressed envelope encode / decode が存在しないため、繰り返し出現する actor path と manifest を短縮できない。

## 変更内容

- actor ref と serializer manifest 用の no_std compression table state を `remote-core` に追加し、literal 値、table id、generation、ack 済み状態、hit count を保持できるようにする。
- `ControlPdu` に compression table advertisement と acknowledgement を追加し、peer 間で table entry を同期する。
- `EnvelopePdu` の recipient actor path、sender actor path、serializer manifest を literal または table reference として表現できる wire metadata に変更する。
- `RemoteCompressionConfig` の actor-ref / manifest max と advertisement interval を、設定保持だけでなく table sizing と advertisement scheduling に反映する。
- std TCP adaptor は monotonic timer で compression advertisement を起動し、remote-core の no_std table 型を保持して、ack 済み table entry を outbound envelope conversion / inbound frame handling に適用する。
- payload bytes 自体の圧縮、Pekko Artery との byte compatibility、remote deployment daemon、serializer registry の契約変更はこの change の対象外とする。

## Capabilities

### 新規 Capabilities

- `remote-wire-compression`: actor ref と serializer manifest の compression table state、advertisement / acknowledgement、hit counting、literal fallback の意味論を定義する。

### 変更する Capabilities

- `remote-core-settings`: compression settings を settings-only ではなく wire-level compression の table sizing / advertisement scheduling に反映する。
- `remote-core-wire-format`: `ControlPdu` と `EnvelopePdu` に fraktor-native compression table advertisement / reference 表現を追加する。
- `remote-adaptor-std-tcp-transport`: std TCP transport が compression advertisement timer と envelope encode/decode の table 適用を行う。

## 影響範囲

- `modules/remote-core/src/config/`
- `modules/remote-core/src/wire/`
- `modules/remote-adaptor-std/src/transport/tcp/`
- `modules/remote-adaptor-std/src/extension_installer/`
- `docs/gap-analysis/remote-gap-analysis.md`

public wire layout は fraktor-native format 内で変更されるが、Pekko Artery byte compatibility は引き続き非目標である。`remote-core` は `std` に依存せず、table state と wire metadata は `core` / `alloc` の範囲で実装する。
