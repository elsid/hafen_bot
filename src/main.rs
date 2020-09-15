#![feature(duration_saturating_ops)]
#![feature(duration_zero)]

#[macro_use]
extern crate hexf;
#[macro_use]
extern crate log;

use self::bot::{read_config, run_server};

mod bot;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    env_logger::init();
    let path = args.get(1).map(|v| v.as_str()).unwrap_or("etc/config.yaml");
    info!("Read config from: {}", path);
    run_server(read_config(path)?)?.await
}
