use std::collections::HashMap;

use cors::cors;
#[cfg(debug_assertions)]
use dotenv::dotenv;
use rocket::{Build, Config, Rocket, Route, launch};

mod cors;
mod error;

#[launch]
async fn rocket() -> _ {
  #[cfg(debug_assertions)]
  dotenv().ok();

  let url = std::env::var("LOKI_URL").expect("Failed to load LOKI_URL");
  let level = std::env::var("RUST_LOG")
    .unwrap_or("warn".into())
    .parse()
    .expect("Failed to parse RUST_LOG");
  let application = std::env::var("LOKI_APP").expect("Failed to load LOKI_APP");
  let environment = std::env::var("LOKI_ENV").expect("Failed to load LOKI_ENV");
  let log_labels = HashMap::from_iter([
    ("application".into(), application),
    ("environment".into(), environment),
  ]);
  loki_logger::init_with_labels(url, level, log_labels).expect("Failed to init logger");

  let cors = cors();

  let figment = Config::figment()
    .merge(("address", "0.0.0.0"))
    .merge(("log_level", "normal"));

  let server = rocket::custom(figment)
    .attach(cors)
    .manage(rocket_cors::catch_all_options_routes())
    .mount("/", routes());

  state(server)
}

fn routes() -> Vec<Route> {
  rocket::routes![dummy]
}

#[rocket::get("/dummy")]
fn dummy() -> String {
  "dummy".into()
}

fn state(server: Rocket<Build>) -> Rocket<Build> {
  server
}
