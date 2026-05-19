# remote-adaptor-std のラッパー構造排除プラン

## 概要

`remote-adaptor-std` から `StdRemoting` という core の代替入口をなくし、ユーザーが `remote-core` の API に std アダプタを差し込んで使う構造へ揃える。DDD の集約としては扱わず、`remote-core` 側に通常の公開実行体を置き、std 側は Port 実装と runtime driver に限定する。

後方互換は考慮しない。`StdRemoting` や `remoting()` のフォールバック、非推奨エイリアス、互換 shim は作らない。

## 主要変更

- `remote-core::core::extension` に非ジェネリックな `Remote` を追加し、`Remoting` trait の標準実装を core 側へ移す。
- `Remote` は lifecycle、transport Port、config、event publisher、advertised addresses のみを持つ。association / watcher / tokio task の std 実行詳細は持たせない。
- `Remote` は `RemoteTransport` を `Box<dyn RemoteTransport + Send>` として内部に保持し、`TcpRemoteTransport` などのアダプタ実装型を利用側の型シグネチャへ露出しない。
- `remote-adaptor-std` の `StdRemoting` を削除する。`extension_installer` は `TcpRemoteTransport` を `Remote` に差し込んで登録するだけにする。
- `RemotingExtensionInstaller::remoting()` は削除し、`remote()` で `SharedLock<Remote>` を返す形へ置換する。
- `remote-adaptor-std` の rustdoc から `aggregate`, `StdRemoting`, `legacy god-object counterpart` という説明を削除し、「std は core Port の具象実装」という説明へ統一する。

## 実装詳細

- `modules/remote-core/src/core/extension/remote.rs` を追加し、`Remote` を 1 公開型 1 ファイルで定義する。
- `Remote::new<T>(transport, config, event_publisher)` は具体 transport を受け取るが、保持型は `Box<dyn RemoteTransport + Send>` にする。ジェネリックはコンストラクタ境界だけに閉じ込める。
- `Remote` の API は最小限にする: `new`, `lifecycle`, `config` と `Remoting` trait 実装のみ。transport 参照アクセサは公開しない。
- `modules/remote-adaptor-std/src/std/extension_installer/base.rs` を削除し、`StdRemoting` の公開 re-export も削除する。
- `RemotingExtensionInstaller` は `TcpRemoteTransport` と `RemoteConfig` から `Remote` を作る。`EventPublisher` は install 時に actor system から作る。
- `AssociationRegistry` / `AssociationShared` は runtime driver の内部状態として扱う。外部公開が不要なものは `pub(crate)` へ落とし、core の通常 API 入口にはしない。
- `TcpRemoteTransport` と `StdRemoteActorRefProvider` は Port 実装なので残す。ここは「ラッパー」ではなく adapter 本体として扱う。

## テスト計画

- `rtk cargo test -p fraktor-remote-core-rs`
- `rtk cargo test -p fraktor-remote-adaptor-std-rs`
- remote installer を利用している cluster 側の参照があれば、該当パッケージのテストも実行する。
- ソース編集を行った最終段階で `./scripts/ci-check.sh ai all` を実行し、完了まで待つ。

## 前提

- 後方互換は不要。既存の `StdRemoting` 利用箇所はすべて新 API に置換する。
- 互換 shim、deprecated alias、旧メソッド名の残置はしない。
- DDD の集約という設計語彙は使わない。責務は「core API」と「std adapter」の境界で整理する。
