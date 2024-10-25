use poem::web::websocket::WebSocket;
use poem::web::Path;
use poem::{Endpoint, EndpointExt, Route};

pub struct LumioConfig {
    pub op_sol: url::Url,
    pub op_move: url::Url,
}

pub struct Lumio {}

#[derive(Clone, Copy)]
pub struct LumioState {}

#[poem::handler]
async fn lumio_attrs_since(Path(slot): Path<u64>, ws: WebSocket) {}

impl Lumio {
    pub fn new(cfg: LumioConfig) -> (Self, impl Endpoint + Send + Sync) {
        let route = Route::new()
            .at("/attrs/since/:slot", poem::get(lumio_attrs_since))
            .with(poem::middleware::AddData::new(LumioState {}));
        (Self {}, route)
    }
}
