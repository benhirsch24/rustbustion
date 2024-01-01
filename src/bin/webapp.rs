use actix_web::{get, error, web, App, HttpServer};
use aws_sdk_s3::Client;
use bytes::{BytesMut};
use chrono::prelude::*;
use std::sync::{Arc, Mutex};

#[get("/")]
async fn index(data: web::Data<Arc<Mutex<State>>>) -> actix_web::Result<String> {
    let data = data.lock().unwrap();

    // Get the last folder which is the latest cook
    let dir = match get_dir(&data.client, &data.bucket).await.map_err(|e| error::ErrorInternalServerError(e))? {
        Some(d) => d,
        None => return Ok(format!("No directories {}", data.bucket)),
    };

    let obj = match get_last_obj(&data.client, &data.bucket, &dir).await.map_err(|e| error::ErrorInternalServerError(e))? {
        Some(o) => o,
        None => return Ok(format!("No objects {}/{}", data.bucket, dir)),
    };

    let contents = read_obj(&data.client, &data.bucket, &obj).await.map_err(|e| error::ErrorInternalServerError(e))?;
    let contents = std::str::from_utf8(&contents).unwrap();
    let first = contents.split("\n")
        .next()
        .ok_or("nothing in this file")
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let parts: Vec<&str> = first.split(",").collect();
    if parts.len() != 2 {
        return Ok(format!("expected 2 parts got {}: {}", parts.len(), contents));
    }
    let temp = parts[0].parse::<f32>().map_err(|e| error::ErrorInternalServerError(e))?;
    let dt = chrono::DateTime::parse_from_rfc3339(parts[1]).map_err(|e| error::ErrorInternalServerError(e))?;

    Ok(format!("{} {}", temp, Utc::now().signed_duration_since(dt).num_minutes()))
}

struct State {
    client: Client,
    bucket: String,
}

impl State {
    pub fn new(client: Client, bucket: String) -> State {
        State {
            client,
            bucket,
        }
    }
}

async fn get_dir(client: &Client, bucket: &String) -> anyhow::Result<Option<String>> {
    let mut dir = None;
    let mut response = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .delimiter("/")
        .into_paginator()
        .send();
    while let Some(result) = response.next().await {
        let response = match result {
            Ok(r) => r,
            Err(e) => {
                anyhow::bail!("Failed fetching objects from bucket: {:?}", e);
            }
        };

        let dirs = response.common_prefixes();
        let d = &dirs[dirs.len()-1];
        dir = d.prefix.clone();
    }

    Ok(dir)
}

async fn get_last_obj(client: &Client, bucket: &str, dir: &str) -> anyhow::Result<Option<String>> {
    let mut last = None;
    let mut response = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .prefix(dir.to_owned())
        .into_paginator()
        .send();

    while let Some(result) = response.next().await {
        let response = match result {
            Ok(r) => r,
            Err(e) => {
                anyhow::bail!("Failed fetching objects from bucket: {:?}", e);
            }
        };

        let objs = response.contents();
        last = objs[objs.len()-1].key.clone();
    }

    Ok(last)
}

async fn read_obj(client: &Client, bucket: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let mut response = client
        .get_object()
        .bucket(bucket.to_owned())
        .key(key.to_owned())
        .send()
        .await?;

    let mut bs = BytesMut::new();
    while let Some(bytes) = response.body.try_next().await? {
        bs.extend_from_slice(&bytes.to_vec())
    }

    Ok(bs.freeze().to_vec())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let flags = xflags::parse_or_exit! {
        /// Bucket to upload data into
        required bucket: String
    };

    println!("{}", flags.bucket);

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    // Get the last folder which is the latest cook
    //let dir = get_dir(&client, &flags.bucket).await.unwrap().unwrap();
    //let obj = get_last_obj(&client, &flags.bucket, &dir).await.unwrap().unwrap();
    //let contents = read_obj(&client, &flags.bucket, &obj).await.unwrap();
    //println!("{}", std::str::from_utf8(&contents).unwrap());

    let state = Arc::new(Mutex::new(State::new(client, flags.bucket)));
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(index)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
