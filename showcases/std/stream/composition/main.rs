use std::{error::Error, time::Duration};

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_kernel_rs::{
  dsl::{Flow, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() -> Result<(), Box<dyn Error>> {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config)?;
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start()?;
  // 失敗時にも materializer.shutdown() を必ず通すため、実行本体をクロージャに閉じる
  let outcome = {
    let mut run = || -> Result<(), Box<dyn Error>> {
      let graph = Source::from_array([1_u32, 2])
        .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
        .into_mat(Sink::collect(), KeepRight);
      let materialized = graph.run(&mut materializer)?;
      let values = materialized.materialized().wait_blocking(&StdBlocker::new())?;
      assert_eq!(values, vec![1, 2, 3, 4]);
      println!("stream_composition collected values: {values:?}");
      Ok(())
    };
    run()
  };
  let shutdown_result = materializer.shutdown();
  // 実行エラーを優先して報告する。両方失敗した場合、shutdown 側のエラーは実行失敗の帰結のため省く
  outcome?;
  shutdown_result?;
  Ok(())
}
