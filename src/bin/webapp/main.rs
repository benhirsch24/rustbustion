use actix_web::{get, http::header::ContentType, error, web, App, HttpResponse, HttpServer};
use aws_sdk_s3::Client;
use log::{error, info};
use chrono::prelude::*;
use handlebars::Handlebars;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

mod s3;
use s3::*;

fn as_farenheit(c: f32) -> f32 {
    c * 1.8 + 32.0
}

#[get("/")]
async fn index(data: web::Data<Arc<Mutex<State>>>) -> actix_web::Result<HttpResponse> {
    let update = {
        let data = data.lock().unwrap();
        if data.last_update.is_none() {
            return Err(error::ErrorInternalServerError(anyhow::anyhow!("No update")));
        }
        data.last_update.clone().unwrap()
    };

    let hb = Handlebars::new();
    let mut data = BTreeMap::new();
    data.insert("temperature".to_string(), format!("{}°F", as_farenheit(update.temp)));
    data.insert("last_update".to_string(), update.time.to_rfc3339_opts(SecondsFormat::Millis, true));
    data.insert("since".to_string(), format!("{} minutes ago", Utc::now().signed_duration_since(update.time).num_minutes()));
    let templ = include_str!("static/index.html.tmpl");

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(hb.render_template(templ, &data).map_err(|e| error::ErrorInternalServerError(e))?))
}


#[get("/health")]
async fn health() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body("ok"))
}

#[derive(Default, Debug)]
struct State {
    last_update: Option<LastUpdate>,
}

impl State {
    fn set_update(&mut self, update: LastUpdate) {
        self.last_update.replace(update);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let flags = xflags::parse_or_exit! {
        /// Bucket to upload data into
        required bucket: String
    };
    env_logger::init();

    let state = Arc::new(Mutex::new(State::default()));

    tokio::spawn({
        let state = state.clone();
        async move {
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let client = Client::new(&config);
            let bucket = flags.bucket;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(60000));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let update = get_last_update(&client, &bucket).await;
                        match update {
                            Err(e) => error!("Error updating last temperature: {e:?}"),
                            Ok(u) => {
                                state.lock().unwrap().set_update(u);
                                info!("Updated");
                            }
                        }
                    }
                }
            }
        }
    });

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(index)
            .service(health)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
