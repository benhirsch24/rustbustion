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
use combustion::CombustionFinder;

mod push;
use push::Pusher;

fn as_farenheit(c: f32) -> f32 {
    c * 1.8 + 32.0
}

#[derive(Clone, Copy, Debug)]
enum SvcStatus {
    DISCOVERING,
    CONNECTING,
    CONNECTED,
    RUNNING,
}

impl std::fmt::Display for SvcStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let value = match self {
            SvcStatus::DISCOVERING => "discovering".to_string(),
            SvcStatus::CONNECTING => "connecting".to_string(),
            SvcStatus::CONNECTED => "connected".to_string(),
            SvcStatus::RUNNING => "running".to_string(),
        };
        write!(f, "{}", value)
    }
}

#[derive(Debug)]
struct SvcInner {
    raw_temp: f32,
    status: SvcStatus,
}

#[derive(Debug, Clone)]
struct Svc {
    inner: Arc<Mutex<SvcInner>>,
}

impl Svc {
    pub fn get_status(&self) -> SvcStatus {
        self.inner.lock().unwrap().status
    }

    pub fn get_raw_temp(&self) -> f32 {
        self.inner.lock().unwrap().raw_temp
    }

    pub fn set_status(&self, status: SvcStatus) {
        self.inner.lock().unwrap().status = status;
    }

    pub fn set_raw_temp(&self, raw_temp: f32) {
        self.inner.lock().unwrap().raw_temp = raw_temp;
    }
}

impl HyperService<Request<IncomingBody>> for Svc {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        fn mk_response(s: String) -> Result<Response<Full<Bytes>>, hyper::Error> {
            Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
        }

        let res = match req.uri().path() {
            "/" => {
                mk_response(format!("{{ \"temp\": {}, \"status\": \"{}\" }}", self.get_raw_temp(), self.get_status()))
            },
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
        optional bucket: String
    };
    info!("Using bucket {:?}", flags.bucket);

    // Listen for Ctrl-C
    let (done_tx, mut done) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("ctrlc");
        done_tx.send(true).expect("Send ctrlc");
    });

    let svc = Svc{ inner: Arc::new(Mutex::new(SvcInner{raw_temp: 0.0, status: SvcStatus::DISCOVERING})) };

    // Start an HTTP server to serve requests for current temp data
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], 3000).into();
    let listener: tokio::net::TcpListener = tokio::net::TcpListener::bind(addr).await.expect("Tcp listener");
    tokio::spawn({
        let svc = svc.clone();
        async move {
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

    svc.set_status(SvcStatus::CONNECTING);

    info!("Connecting to device");
    if let Err(e) = combustion.connect().await {
        error!("Could not connect to device: {:?}", e);
        return Err(e);
    }

    svc.set_status(SvcStatus::CONNECTED);

    // Create the Pusher
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        let mut pusher = Pusher::new();
        if let Some(bucket) = flags.bucket {
            let prefix = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
            info!("Starting S3 pusher to {}/{}", bucket, prefix);
            pusher.init(bucket, prefix).await;
        }

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
                        svc.set_status(SvcStatus::RUNNING);
                        svc.set_raw_temp(as_farenheit(raw_temp_c));
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
