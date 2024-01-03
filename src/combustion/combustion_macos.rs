#[cfg(target_os="macos")]
pub mod macos {
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
