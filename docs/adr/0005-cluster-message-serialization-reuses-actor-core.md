# Cluster message serialization は actor-core serialization を再利用する

Cluster Message Serialization (クラスタメッセージ直列化) は actor-core の `SerializedMessage` metadata を source of truth とし、その外側に cluster payload kind と versioned wire frame を重ねる。cluster 専用の第二 serializer registry を作らず actor-core serialization と診断を揃えるため、未対応 frame version や未知の payload kind は silent fallback ではなく明示的な revalidation point として扱う。
