#![cfg_attr(not(feature = "unsize"), allow(dead_code))]

#[cfg(not(feature = "unsize"))]
compile_error!(
  "`unsize` フィーチャが無効の場合、このサンプルはビルドできません。`--features unsize` を指定してください。"
);

use cellactor_utils_core_rs::ArcShared;

trait Greeter {
  fn greet(&self) -> &'static str;
}

struct SimpleGreeter;

impl Greeter for SimpleGreeter {
  fn greet(&self) -> &'static str {
    "こんにちは"
  }
}

fn main() {
  // 具象型の ArcShared からトレイトオブジェクトへの暗黙の Unsized 変換を確認する
  let concrete: ArcShared<SimpleGreeter> = ArcShared::new(SimpleGreeter);
  let trait_object: ArcShared<dyn Greeter> = concrete;

  assert_eq!(trait_object.greet(), "こんにちは");

  // into_dyn に頼らず、普通の参照変換と同じ感覚で動的ディスパッチを活用できる
  println!("{}", trait_object.greet());
}
