mod prelude {
  pub fn prelude_fn() {}
}

pub use prelude::prelude_fn;

pub type Alias<T> = Option<T>;

use prelude::prelude_fn as _;

fn main() {}
