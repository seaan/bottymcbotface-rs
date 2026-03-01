pub mod dnd;

use std::{env, sync::Arc};

use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::data::dnd::DndRepository;

pub async fn run_server(
    dnd_repo: Arc<DndRepository>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind_addr = env::var("DND_WEB_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".to_string());

    let app = axum::Router::new()
        .merge(dnd::router(dnd::DndWebState { dnd: dnd_repo }))
        .nest_service("/assets", ServeDir::new("src/web/assets"));

    let listener = TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
