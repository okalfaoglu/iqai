//! IQAI Web GUI — binary; router `iqai_web::http_app`.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    iqai_web::run_server().await
}
