#![cfg_attr(feature = "unsize", feature(unsize, coerce_unsized))]

/// `unsize` featureが有効な場合にのみ実演コードを実行する。
#[cfg(not(feature = "unsize"))]
fn main() {
  println!("Enable the `unsize` feature on nightly to run this example.");
}

#[cfg(feature = "unsize")]
use cellactor_utils_core_rs::ArcShared;

#[cfg(feature = "unsize")]
trait HogeTrait {
  fn run(&self);
}

#[cfg(feature = "unsize")]
struct Hoge;

#[cfg(feature = "unsize")]
impl HogeTrait for Hoge {
  fn run(&self) {
    println!("Hello!");
  }
}

#[cfg(feature = "unsize")]
fn main() {
  let x: ArcShared<Hoge> = ArcShared::new(Hoge);
  // 明示的な into_dyn 呼び出しなしで型強制が働くことを確認
  let y: ArcShared<dyn HogeTrait> = x;
  y.run();
}
