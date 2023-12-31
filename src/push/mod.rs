use aws_sdk_s3::Client;
use aws_smithy_types::byte_stream::ByteStream;
use bytes::Bytes;
use chrono::prelude::*;

const BATCH_SIZE: usize = 5;

struct Update {
    temp: f32,
    time: DateTime<Utc>,
}

pub struct Pusher {
    client: Client,
    bucket: String,
    prefix: String,
    key: u32,
    window: std::vec::Vec<Update>,
}

impl Pusher {
    pub async fn new(bucket: String, prefix: String) -> Pusher {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        Pusher{
            client,
            bucket,
            prefix,
            key: 0,
            window: vec![],
        }
    }

    pub async fn push(&mut self, t: f32) -> anyhow::Result<()> {
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

        self.client
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
