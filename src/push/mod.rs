use aws_sdk_s3::Client;
use aws_smithy_types::byte_stream::ByteStream;
use bytes::Bytes;
use chrono::prelude::*;

const BATCH_SIZE: usize = 1000;

struct Update {
    temp: f32,
    time: DateTime<Utc>,
}

pub struct Pusher {
    client: Option<Client>,
    bucket: String,
    prefix: String,
    key: u32,
    window: std::vec::Vec<Update>,
}

impl Pusher {
    pub fn new() -> Pusher {
        Pusher{
            client: None,
            bucket: String::new(),
            prefix: String::new(),
            key: 0,
            window: vec![],
        }
    }

    pub async fn init(&mut self, bucket: String, prefix: String) {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        self.bucket = bucket;
        self.prefix = prefix;
        self.client = Some(client);
    }

    pub async fn push(&mut self, t: f32) -> anyhow::Result<()> {
        if self.client.is_none() {
            return Ok(());
        }

        let client = self.client.as_ref().unwrap();
        // If we've exceeded the window size then clear it and increment the key
        if self.window.len() > BATCH_SIZE {
            self.window.clear();
            self.key += 1;
        }

        // Push the new temp into the window and serialize it
        self.window.push(Update{temp: t, time: Utc::now()});
        let obj = self.serialize();

        // Upload to S3
        let key = format!("{}/{}.csv", self.prefix, self.key);
        log::debug!("Uploading {} to {}", obj, key);

        client
            .put_object()
            .bucket(self.bucket.clone())
            .key(key)
            .body(ByteStream::from(Bytes::from(obj)))
            .send()
            .await?;
        Ok(())
    }

    fn serialize(&self) -> String {
        // Format is "temp,datetime" joined by new lines where the latest value is the first
        self.window
            .iter()
            .rev()
            .map(|v| {
                format!("{},{}", v.temp, v.time.to_rfc3339_opts(SecondsFormat::Millis, true))
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}
