use colored::Colorize;
use xtrade::{AppResult, cli::Cli, config::Config, init_logging, session::SessionManager};

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse_args();

    // Initialize logging
    init_logging(&cli.effective_log_level())?;

    tracing::info!("XTrade Market Data Monitor starting...");
    tracing::debug!("CLI arguments: {:?}", cli);

    // Load configuration
    let config = Config::load_or_default(&cli.config_file);

    // Create session manager
    let mut session_manager = SessionManager::new(&cli, config)?;

    // Initialize session
    session_manager.initialize().await?;

    // Run interactive session
    session_manager.run().await?;

    Ok(())
}
