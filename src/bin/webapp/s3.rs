use aws_sdk_s3::Client;
use anyhow::{anyhow, bail};
use chrono::prelude::*;
use bytes::{BytesMut};

#[derive(Clone, Default, Debug)]
pub struct LastUpdate {
    pub temp: f32,
    pub time: DateTime<FixedOffset>,
}

pub async fn get_last_update(client: &Client, bucket: &str) -> anyhow::Result<LastUpdate> {
    // Get the last folder which is the latest cook
    let dir = match get_dir(client, bucket).await? {
        Some(d) => d,
        None => bail!("No directories in {}", bucket)
    };

    let obj = match get_last_obj(client, bucket, &dir).await? {
        Some(o) => o,
        None => bail!("No objects in {}/{}", bucket, dir)
    };

    let contents = read_obj(client, bucket, &obj).await?;
    let first = contents.split("\n")
        .next()
        .ok_or(anyhow!("nothing in this file"))?;
    let parts: Vec<&str> = first.split(",").collect();
    if parts.len() != 2 {
        bail!("expected 2 parts got {}: {}", parts.len(), contents);
    }
    let temp = parts[0].parse::<f32>()?;
    let dt = chrono::DateTime::parse_from_rfc3339(parts[1])?;

    Ok(LastUpdate{temp: temp, time: dt})
}

async fn get_dir(client: &Client, bucket: &str) -> anyhow::Result<Option<String>> {
    // List objects in the bucket with "/" as the delimiter to find the top level directories
    let mut dir = None;
    let mut response = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .delimiter("/")
        .into_paginator()
        .send();

    // Get the last directory in the list as RFC3339 should sort these lexicographically
    while let Some(result) = response.next().await {
        let response = match result {
            Ok(r) => r,
            Err(e) => {
                anyhow::bail!("Failed fetching objects from bucket: {:?}", e);
            }
        };

        // Common prefixes with delimiter "/" returns the top level dirs
        let dirs = response.common_prefixes();
        if dirs.len() == 0 {
            break;
        }
        let d = &dirs[dirs.len()-1];
        dir = d.prefix.clone();
    }

    Ok(dir)
}

async fn get_last_obj(client: &Client, bucket: &str, dir: &str) -> anyhow::Result<Option<String>> {
    // Get the objects in the bucket with the directory prefix
    let mut last = None;
    let mut response = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .prefix(dir.to_owned())
        .into_paginator()
        .send();


    // Get the last one (has the most recent update)
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

async fn read_obj(client: &Client, bucket: &str, key: &str) -> anyhow::Result<String> {
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

    // Probably can do this more efficiently, but that's ok for now
    let bytevec = bs.freeze().to_vec();
    let contents = std::str::from_utf8(&bytevec)?;
    Ok(contents.to_string())
}
