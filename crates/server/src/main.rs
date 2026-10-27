#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tv_manager_server::run_from_env().await
}
