#![feature(duration_saturating_ops)]
#![feature(duration_zero)]

#[macro_use]
extern crate hexf;
#[macro_use]
extern crate log;

use self::bot::run_server;

mod bot;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    run_server()?.await
}
