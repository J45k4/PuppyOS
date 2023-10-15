use std::{convert::Infallible, net::SocketAddr};

use anyhow::bail;
use hyper::{service::{make_service_fn, service_fn}, Response, Body, Request, StatusCode};
use log::LevelFilter;
use mime_guess::from_path;
use simple_logger::SimpleLogger;
use hyper::Server;
use std::path::Path;

macro_rules! serve_static_file {
    ($path:expr) => {{
        let body = match std::option_env!("STATIC_ASSETS") {
            Some(_) => {
                log::debug!("Using included asset");
                Body::from(include_str!($path))
            },
            None => {
                log::debug!("Using file asset");
                let path = Path::new("./src").join($path);
                Body::from(tokio::fs::read_to_string(path).await?)
            }
        };

        if $path.ends_with(".js") {
            Response::builder()
                .header("Content-Type", "application/javascript")
                .body(body)?
        } else if $path.ends_with(".css") {
            Response::builder()
                .header("Content-Type", "text/css")
                .body(body)?
        } else if $path.ends_with(".html") {
            Response::builder()
                .header("Content-Type", "text/html")
                .body(body)?
        } else {
            Response::new(body)
        }
    }};
}

pub async fn handle_request(mut req: Request<Body>) -> Result<Response<Body>, anyhow::Error> {
    log::info!("{} {}", req.method(), req.uri());
    log::debug!("agent: {:?}", req.headers().get("user-agent"));

    if hyper_tungstenite::is_upgrade_request(&req) {
        log::info!("there is upgrade request");
        let (response, ws) = hyper_tungstenite::upgrade(&mut req, None)?;
        log::info!("websocket upgraded");

        match req.uri().path() {
            "/ws" => {
                log::debug!("new ws request");

                // tokio::spawn(async move {
                //     let ws = ws.await.unwrap();
                //     WsServer::new(ws, ctx.clone()).serve().await;
                // });
            }
            _ => {
                bail!("not allowed url")
            }
        };

        return Ok(response);
    }

    let path = req.uri().path().replace("/PuppyOS", "");

    let static_path = format!("./static{}", path);
    log::debug!("static_path: {:?}", static_path);
    let static_path = Path::new(&static_path);
    if static_path.exists() && static_path.is_file() {
        let content = tokio::fs::read(&static_path).await?;
        let mime_type = mime_guess::from_path(&static_path).first_or_octet_stream();
    
        let res = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_type.as_ref())
            .body(Body::from(content))
            .unwrap();
        return Ok(res);
    }

    match path.trim() {
        _ => {
            log::debug!("using index.html");
            Ok(serve_static_file!("../static/index.html"))
        }
    }
}

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

    let make_scv = make_service_fn(move |_| {
        // let ctx = ctx.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                // let ctx = ctx.clone();
                async move {
                    match handle_request(req).await {
                        Ok(res) => {
                            Ok::<_, Infallible>(res)
                        },
                        Err(e) => {
                            log::error!("request error: {}", e);
                            Ok::<_, Infallible>(Response::new(Body::from("error")))
                        }
                    }
                }
            }))
        }
    });

    log::info!("listen port: 8551");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8551));
    Server::bind(&addr).serve(make_scv).await.unwrap();
}
