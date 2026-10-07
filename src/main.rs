use clap::Parser;
use msgs::app::{self, Options};

#[tokio::main]
async fn main() {
    if let Err(error) = app::run(Options::parse()).await {
        eprintln!("msgs: {error:#}");
        std::process::exit(1);
    }
}
