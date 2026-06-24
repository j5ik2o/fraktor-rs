# Cluster message serialization は actor-core serialization を再利用する

Cluster Message Serialization (クラスタメッセージ直列化) は actor-core の `SerializedMessage` metadata を source of truth とし、その外側に cluster payload kind と versioned wire frame を重ねる。cluster 専用の第二 serializer registry を作らず actor-core serialization と診断を揃えるため、未対応 frame version や未知の payload kind は silent fallback ではなく明示的な revalidation point として扱う。

**Considered Options**

- cluster 専用 serializer registry を追加する案: actor-core serialization と診断経路が二重化し、payload kind ごとの不一致を増やすため不採用。
- cluster protocol ごとに raw bytes を直接扱う案: wire shape は単純になるが、serializer id / manifest / diagnostics を actor-core と共有できなくなるため不採用。
- actor-core の `SerializedMessage` metadata に cluster payload kind と versioned wire frame を重ねる案: 既存 serialization contract を source of truth にしながら cluster 固有の payload 判別を明示できるため採用。
