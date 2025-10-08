use xtrade::{
    AppResult,
    binance::demo,
    cli::{Cli, Commands},
    config::Config,
    init_logging,
    session::SessionManager,
};

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse_args();

    // Initialize logging
    init_logging(&cli.effective_log_level())?;

    tracing::info!("XTrade Market Data Monitor starting...");
    tracing::debug!("CLI arguments: {:?}", cli);

    // Get command
    let command = &cli.command();
    match command {
        Commands::Config { action } => {
            Config::handle_command(action)?;
            Ok(())
        }
        Commands::Demo => demo::demo_websocket().await,
        _ => {
            let config = Config::load_or_default(&cli.config_file);
            let mut session_manager = SessionManager::new(&cli, config)?;
            session_manager.start().await?;
            Ok(())
        }
    }
}
