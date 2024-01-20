use actix_web::{get, http::header::ContentType, error, web, App, HttpResponse, HttpServer};
use aws_sdk_s3::Client;
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
        get_last_update(&data.client, &data.bucket)
            .await
            .map_err(|e| error::ErrorInternalServerError(e))
    }?;

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let flags = xflags::parse_or_exit! {
        /// Bucket to upload data into
        required bucket: String
    };
    env_logger::init();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let state = Arc::new(Mutex::new(State::new(client, flags.bucket)));
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
