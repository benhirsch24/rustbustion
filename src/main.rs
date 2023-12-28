use bluer::{Address, gatt::remote::{Service}, Device};
use futures::{pin_mut, StreamExt};
use log::{error, info, trace, warn};
use modular_bitfield::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::oneshot::{Receiver};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming as IncomingBody};
use hyper::server::conn::http1;
use hyper::service::{Service as HyperService};
use hyper::{Request, Response};
use std::future::Future;
use std::pin::Pin;

const COMBUSTION_ID: u16 = 0x09C7;
const PROBE_STATUS_SERVICE_UUID: &str = "00000100-CAAB-3792-3D44-97AE51C1407A";
const UART_SERVICE_UUID: &str = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E";

struct Combustion {
    probe_status_service: Service,
    uart_service: Service,
}

#[bitfield]
struct RawTempData {
    t1: B13,
    t2: B13,
    t3: B13,
    t4: B13,
    t5: B13,
    t6: B13,
    t7: B13,
    t8: B13,
}

fn as_farenheit(c: f32) -> f32 {
    c * 1.8 + 32.0
}

async fn check_probe_status(_device: &Device, combustion: &Combustion, last_temp: Arc<Mutex<f32>>, mut done: &mut Receiver<bool>) -> bluer::Result<()> {
    for c in combustion.probe_status_service.characteristics().await? {
        let uuid = c.uuid().await?;
        info!("Probe Status c: {}", &uuid);
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(2000));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // If the characteristic flags includes "read"
                    if c.flags().await?.read {
                        // Read it
                        let value = c.read().await?;

                        let min_bytes: [u8; 4] = [value[0], value[1], value[2], value[3]];
                        let max_bytes: [u8; 4] = [value[4], value[5], value[6], value[7]];
                        let min = u32::from_ne_bytes(min_bytes);
                        let max = u32::from_ne_bytes(max_bytes);
                        info!("Min {} max {}", min, max);

                        let vs: [u8; 13] = value[8..21].try_into().expect("13");
                        let unpacked = RawTempData::from_bytes(vs);
                        let t1c = (unpacked.t1() * 5 - 2000) as f32 / 100.0;
                        info!("T1 {} deg C={} degF={}", unpacked.t1(), t1c, as_farenheit(t1c));
                        {
                            *last_temp.lock().unwrap() = as_farenheit(t1c);
                        }
                    }
                }
                _d = &mut done => {
                    info!("Done!");
                    return Ok(())
                }
            }
        }
    }
    Ok(())
}

async fn connect(device: &Device) -> bluer::Result<Option<Combustion>> {
    let probe_uuid = bluer::Uuid::parse_str(PROBE_STATUS_SERVICE_UUID).expect("probe uuid");
    let uart_uuid = bluer::Uuid::parse_str(UART_SERVICE_UUID).expect("uart uuid");

    sleep(Duration::from_secs(2)).await;
    if !device.is_connected().await? {
        info!("Connecting");
        let mut retries = 2;
        loop {
            match device.connect().await {
                Ok(()) => break,
                Err(err) if retries > 0 => {
                    info!("  Connect error: {}", err);
                    retries -= 1;
                }
                Err(err) => return Err(err),
            }
        }
        info!("  Connected");
    } else {
        info!("  Already Connected");
    }

    let mut probe_service: Option<Service> = None;
    let mut uart_service: Option<Service> = None;

    for service in device.services().await? {
        let uuid = service.uuid().await?;
        info!("  Service UUID: {} ID: {}", &uuid, service.id());
        if uuid == probe_uuid {
            probe_service.replace(service.clone());
        } else if uuid == uart_uuid {
            uart_service.replace(service.clone());
        }
        info!("  Service data: {:?}", service.all_properties().await?);
    }

    if probe_service.is_some() && uart_service.is_some() {
        return Ok(Some(Combustion{probe_status_service: probe_service.unwrap(), uart_service: uart_service.unwrap()}));
    }

    warn!("Did not get all services");
    Ok(None)
}

async fn is_combustion(device: &Device) -> bluer::Result<bool> {
    sleep(Duration::from_secs(2)).await;
    let addr = device.address();
    let uuids = device.uuids().await?.unwrap_or_default();
    info!("Device {} with service UUIDs {:?}", addr, &uuids);
    let md = device.manufacturer_data().await?;
    info!("Manufacturer data: {:x?}", &md);

    if md.is_none() {
        return Ok(false)
    }

    let md = md.unwrap();
    let data = md.get(&COMBUSTION_ID);
    if data.is_none() {
        return Ok(false)
    }
    let data = data.unwrap();
    info!("Found combustion: {:x?}", data);
    Ok(true)
}

async fn discover_devices(adapter: &bluer::Adapter, mut done: &mut Receiver<bool>) -> bluer::Result<(Device, Address)> {
    let discover = adapter.discover_devices().await?;
    pin_mut!(discover);
    loop {
        tokio::select! {
            evt = discover.next() => {
                if evt.is_none() {
                    return Err(bluer::Error{kind: bluer::ErrorKind::Failed, message: "Couldn't find device".to_string()})
                }
                let evt = evt.unwrap();
                match evt {
                    bluer::AdapterEvent::DeviceAdded(addr) => {
                        let device = adapter.device(addr)?;
                        if is_combustion(&device).await? {
                            info!("Address type: {:?}", device.address_type().await?);
                            return Ok((device, addr))
                        }
                    },
                    _ => trace!("Event: {:?}", evt)
                }
            }
            _ = &mut done => {
                info!("Got done signal");
                return Err(bluer::Error{kind: bluer::ErrorKind::Failed, message: "Terminated early".to_string()})
            }
        }
    }
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
async fn main() -> bluer::Result<()> {
    env_logger::init();

    // Listen for Ctrl-C
    let (tx, mut done) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("ctrlc");
        let _ = tx.send(true).expect("Send ctrlc");
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

    // Find and interact with the thermometer
    info!("Creating bluetooth session");
    let session = bluer::Session::new().await?;
    info!("Getting default adapter");
    let adapter = session.default_adapter().await?;
    info!("Setting powered");
    adapter.set_powered(true).await?;

    info!("Discovering devices");
    let (device, addr) = match discover_devices(&adapter, &mut done).await {
        Ok((d, a)) => {
            info!("Discovered addr: {:?}", a);
            (d, a)
        },
        Err(e) => {
            error!("Failed to discover: {}", e);
            return Err(bluer::Error{kind: bluer::ErrorKind::Failed, message: "Couldn't find device".to_string()})
        }
    };

    info!("Connecting to device");
    match connect(&device).await {
        Ok(None) => info!("Not yet"),
        Err(e) => error!("Error: {:?}", e),
        Ok(Some(c)) => {
            info!("Success!");

            info!("Checking Probe Status");
            check_probe_status(&device, &c, last_temp, &mut done).await?;

            info!("Disconnecting");
            if let Err(e) = device.disconnect().await {
                warn!("Failed to disconnect from device: {}", e);
            }
            adapter.remove_device(addr).await?;
        }
    };
    info!("Done");
    Ok(())
}
