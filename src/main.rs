#[macro_use]
extern crate log;

use hafen_bot::bot::{read_config, run_server};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    env_logger::init();
    let path = args.get(1).map(|v| v.as_str()).unwrap_or("etc/config.yaml");
    info!("Read config from: {}", path);
    run_server(read_config(path)?)?.await
}
