use std::collections::HashMap;

use cors::cors;
#[cfg(debug_assertions)]
use dotenv::dotenv;
use fern::Dispatch;
use rocket::{launch, Build, Config, Rocket, Route};

mod cors;
mod error;

#[launch]
async fn rocket() -> _ {
  #[cfg(debug_assertions)]
  dotenv().ok();

  #[allow(unused)]
  let url = std::env::var("LOKI_URL").expect("Failed to load LOKI_URL");
  let level = std::env::var("RUST_LOG")
    .unwrap_or("warn".into())
    .parse()
    .expect("Failed to parse RUST_LOG");
  let application = std::env::var("LOKI_APP").expect("Failed to load LOKI_APP");
  let environment = std::env::var("LOKI_ENV").expect("Failed to load LOKI_ENV");
  #[allow(unused)]
  let log_labels: HashMap<String, String> = HashMap::from_iter([
    ("application".into(), application),
    ("environment".into(), environment),
  ]);

  Dispatch::new()
    .chain(Box::new(env_logger::builder().build()) as Box<dyn log::Log>)
    //.chain(Box::new(log_loki::LokiBuilder::new(url, log_labels).build()) as Box<dyn log::Log>)
    .level(level)
    .apply()
    .expect("Failed to initialize logger");

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
