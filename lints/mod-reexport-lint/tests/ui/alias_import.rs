mod domain {
  pub mod fuga_impl {
    pub struct FugaImpl;
  }
}

use crate::domain::fuga_impl::FugaImpl as Fuga;

fn main() {
  let _ = Fuga;
}
