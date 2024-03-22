use std::{convert::Infallible, net::SocketAddr};

use anyhow::bail;
use hyper::{service::{make_service_fn, service_fn}, Response, Body, Request, StatusCode};
use log::LevelFilter;
use mime_guess::from_path;
use simple_logger::SimpleLogger;
use hyper::Server;
use window::run_window;
use std::path::Path;

mod window;

#[tokio::main]
async fn main() {
    let filter_level = match std::env::var("PUPPYOS_LOG_LEVEL") {
        Ok(lev) => {
            match lev.as_str() {
                "info" => LevelFilter::Info,
                "debug" => LevelFilter::Debug,
                "error" => LevelFilter::Error,
                _ => LevelFilter::Info
            }
        },
        Err(_) => {
            LevelFilter::Info
        }
    };

    SimpleLogger::new()
        .with_level(filter_level)
        .without_timestamps()
        .init()
        .unwrap();

    run_window("PuppyOS");
}
