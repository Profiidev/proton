use rocket::{Route, get};

pub fn routes() -> Vec<Route> {
  rocket::routes![dummy]
    .into_iter()
    .flat_map(|route| route.map_base(|base| format!("{}{}", "/auth", base)))
    .collect()
}

#[get("/dummy")]
fn dummy() -> String {
  "test".into()
}
