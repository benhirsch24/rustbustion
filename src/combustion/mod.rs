#[cfg(target_os="linux")]
pub mod combustion {
    use bluer::{Address, gatt::remote::{Service}, Device};
    use futures::{pin_mut, StreamExt};
    use log::{error, info, trace, warn};
    use modular_bitfield::prelude::*;
    use std::time::Duration;
    use tokio::time::sleep;
    use tokio::sync::oneshot::{Receiver};

    const COMBUSTION_ID: u16 = 0x09C7;
    const PROBE_STATUS_SERVICE_UUID: &str = "00000100-CAAB-3792-3D44-97AE51C1407A";
    const UART_SERVICE_UUID: &str = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E";

    pub struct CombustionFinder {
        adapter: bluer::Adapter,
    }

    impl CombustionFinder {
        pub async fn new() -> anyhow::Result<CombustionFinder> {
            info!("Creating bluetooth session");
            let session = bluer::Session::new().await?;

            info!("Getting default adapter");
            let adapter = session.default_adapter().await?;

            info!("Setting powered");
            adapter.set_powered(true).await?;

            Ok(CombustionFinder{
                adapter
            })
        }

        pub async fn discover(&self, mut done: &mut Receiver<bool>) -> anyhow::Result<Combustion> {
            let discover = self.adapter.discover_devices().await?;
            pin_mut!(discover);
            loop {
                tokio::select! {
                    evt = discover.next() => {
                        if evt.is_none() {
                            return Err(anyhow::anyhow!("Couldn't find device".to_string()));
                        }
                        let evt = evt.unwrap();
                        match evt {
                            bluer::AdapterEvent::DeviceAdded(addr) => {
                                let device = self.adapter.device(addr)?;
                                if is_combustion(&device).await? {
                                    info!("Address type: {:?}", device.address_type().await?);
                                    return Ok(Combustion::new(
                                            device,
                                            self.adapter.clone(),
                                            addr,
                                    ));
                                }
                            },
                            _ => trace!("Event: {:?}", evt)
                        }
                    }
                    _ = &mut done => {
                        info!("Got done signal");
                        return Err(anyhow::anyhow!("Terminated early".to_string()));
                    }
                }
            }
        }
    }

    async fn is_combustion(device: &Device) -> anyhow::Result<bool> {
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

    pub struct Combustion {
        device: Device,
        adapter: bluer::Adapter,
        addr: Address,
        probe_service: Option<Service>,
        uart_service: Option<Service>,
    }

    impl Combustion {
        pub fn new(device: bluer::Device, adapter: bluer::Adapter, addr: Address) -> Combustion {
            Combustion {
                device,
                adapter,
                addr,
                probe_service: None,
                uart_service: None,
            }
        }

        pub async fn connect(&mut self) -> anyhow::Result<()> {
            let probe_uuid = bluer::Uuid::parse_str(PROBE_STATUS_SERVICE_UUID).expect("probe uuid");
            let uart_uuid = bluer::Uuid::parse_str(UART_SERVICE_UUID).expect("uart uuid");

            sleep(Duration::from_secs(2)).await;
            if !self.device.is_connected().await? {
                info!("Connecting");
                let mut retries = 2;
                loop {
                    match self.device.connect().await {
                        Ok(()) => break,
                        Err(err) if retries > 0 => {
                            info!("  Connect error: {}", err);
                            retries -= 1;
                        }
                        Err(err) => return Err(err.into()),
                    }
                }
                info!("  Connected");
            } else {
                info!("  Already Connected");
            }

            for service in self.device.services().await? {
                let uuid = service.uuid().await?;
                info!("  Service UUID: {} ID: {}", &uuid, service.id());
                if uuid == probe_uuid {
                    self.probe_service.replace(service.clone());
                } else if uuid == uart_uuid {
                    self.uart_service.replace(service.clone());
                }
                info!("  Service data: {:?}", service.all_properties().await?);
            }

            if self.probe_service.is_some() && self.uart_service.is_some() {
                return Ok(());
            }

            Err(anyhow::anyhow!("Couldn't find required services"))
        }

        pub async fn get_raw_temp(&self) -> anyhow::Result<Option<f32>> {
            let svc = self.probe_service.as_ref().ok_or(anyhow::anyhow!("No probe service found"))?;
            for c in svc.characteristics().await? {
                let uuid = c.uuid().await?;
                info!("Probe Status c: {}", &uuid);
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
                    return Ok(Some(t1c))
                }
            }
            Ok(None)
        }


        pub async fn disconnect(&self) -> anyhow::Result<()> {
            info!("Disconnecting");
            if let Err(e) = self.device.disconnect().await {
                warn!("Failed to disconnect from device: {}", e);
            }
            self.adapter.remove_device(self.addr).await?;
            Ok(())
        }
    }
}

#[cfg(target_os="macos")]
pub mod combustion {
    use std::sync::{Arc, Mutex};
    use tokio::sync::oneshot::{Receiver};

    pub struct CombustionFinder {
    }

    impl CombustionFinder {
        pub async fn new() -> anyhow::Result<CombustionFinder> {
            Ok(CombustionFinder{
            })
        }

        pub async fn discover(&self, mut _done: &mut Receiver<bool>) -> anyhow::Result<Combustion> {
            Ok(Combustion::new(rand::random::<f32>() * 10.0 + 20.0))
        }
    }

    pub struct Combustion {
        temp: Arc<Mutex<f32>>,
    }

    impl Combustion {
        pub fn new(temp: f32) -> Combustion {
            let t = Arc::new(Mutex::new(temp));
            let t_c = t.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    {
                        let mut t = t_c.lock().unwrap();
                        // Generate a delta between [-1, 1]
                        let delta = rand::random::<f32>() * 2.0 - 1.0;
                        *t += delta;
                    }
                }
            });
            Combustion {
                temp: t,
            }
        }

        pub async fn connect(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        pub async fn get_raw_temp(&self) -> anyhow::Result<Option<f32>> {
            let t = *self.temp.lock().unwrap();
            Ok(Some(t))
        }


        pub async fn disconnect(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }
}
