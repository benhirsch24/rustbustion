use chrono::prelude::*;
use log::{error, info, warn};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming as IncomingBody};
use hyper::server::conn::http1;
use hyper::service::{Service as HyperService};
use hyper::{Request, Response};
use std::future::Future;
use std::pin::Pin;

mod combustion;
use combustion::combustion::CombustionFinder;

mod push;
use push::Pusher;

fn as_farenheit(c: f32) -> f32 {
    c * 1.8 + 32.0
}

#[derive(Debug, Clone)]
struct TempSvc {
    raw_temp: Arc<Mutex<f32>>,
}

impl HyperService<Request<IncomingBody>> for TempSvc {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        fn mk_response(s: String) -> Result<Response<Full<Bytes>>, hyper::Error> {
            Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
        }

        let res = match req.uri().path() {
            "/" => mk_response(format!("{}", *self.raw_temp.lock().unwrap())),
            _ => return Box::pin(async { mk_response("Whoopsie".into()) }),
        };
        Box::pin(async { res })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let flags = xflags::parse_or_exit! {
        /// Bucket to upload data into
        required bucket: String
    };
    info!("Using bucket {}", flags.bucket);

    // Listen for Ctrl-C
    let (done_tx, mut done) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("ctrlc");
        done_tx.send(true).expect("Send ctrlc");
    });

    let svc = TempSvc{raw_temp: Arc::new(Mutex::new(0.0))};
    let last_temp = svc.raw_temp.clone();

    // Start an HTTP server to serve requests for current temp data
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], 3000).into();
    let listener: tokio::net::TcpListener = tokio::net::TcpListener::bind(addr).await.expect("Tcp listener");
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.expect("Accept");
            let io = hyper_util::rt::tokio::TokioIo::new(tcp);
            let svc_clone = svc.clone();
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new().serve_connection(io, svc_clone).await
                {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    info!("Discovering devices");
    let finder = CombustionFinder::new().await?;
    let mut combustion = match finder.discover(&mut done).await {
        Ok(d) => d,
        Err(e) => {
            error!("Could not find combustion device: {:?}", e);
            return Err(e);
        }
    };

    info!("Connecting to device");
    if let Err(e) = combustion.connect().await {
        error!("Could not connect to device: {:?}", e);
        return Err(e);
    }

    // Create the Pusher
    let key = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut pusher = Pusher::new(flags.bucket, key).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        info!("Starting S3 pusher");
        let mut i = 0;
        while let Some(t) = rx.recv().await {
            // Do something
            if let Err(e) = pusher.push(t).await {
                error!("Failed to push t={} i={}: {}", t, i, e);
            }
            i += 1;
        }
    });

    // Poll the thermometer and push the temps into the S3 Pusher
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(2000));
    let mut i = 0;
    loop {
        tokio::select! {
            _ = interval.tick() => {
                match combustion.get_raw_temp().await? {
                    Some(raw_temp_c) => {
                        info!("Raw temp deg C={} degF={}", raw_temp_c, as_farenheit(raw_temp_c));
                        {
                            *last_temp.lock().unwrap() = as_farenheit(raw_temp_c);
                        }
                        if let Err(e) = tx.send(raw_temp_c).await {
                            error!("Failed to send raw temp={} entry={} to pusher: {}", raw_temp_c, i, e);
                        }
                        i += 1;
                    },
                    None => {
                        warn!("Couldn't fetch temp");
                    }
                }
            }
            _d = &mut done => {
                info!("Done!");
                break;
            }
        }
    }

    if let Err(e) = combustion.disconnect().await {
        error!("Failed to disconnect device: {:?}", e);
    }

    info!("Done");
    Ok(())
}
