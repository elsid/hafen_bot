extern crate log;

use self::bot::run_server;

mod bot;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    run_server()?.await
}
