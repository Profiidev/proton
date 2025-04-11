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

  let level = std::env::var("RUST_LOG")
    .unwrap_or("warn".into())
    .parse()
    .expect("Failed to parse RUST_LOG");

  Dispatch::new()
    .chain(Box::new(env_logger::builder().build()) as Box<dyn log::Log>)
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
