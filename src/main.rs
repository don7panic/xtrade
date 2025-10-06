use colored::Colorize;
use xtrade::{
    AppResult,
    cli::{Cli, Commands, ConfigAction},
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

    // Handle config command directly
    if let Commands::Config { action } = &cli.command() {
        return handle_config_command(action).await;
    }

    // Handle demo command directly
    if let Commands::Demo = &cli.command() {
        return handle_demo_command().await;
    }

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

/// Handle config command directly
async fn handle_config_command(action: &Option<ConfigAction>) -> AppResult<()> {
    match action {
        Some(ConfigAction::Show) => {
            let config = Config::load_or_default("config.toml");
            println!("Current configuration:");
            println!("{:#?}", config);
        }
        Some(ConfigAction::Set { key, value }) => {
            println!("Config set command: {} = {}", key, value);
            println!("Note: Config set functionality not yet implemented");
        }
        Some(ConfigAction::Reset) => {
            let default_config = Config::default();
            println!("Configuration reset to defaults:");
            println!("{:#?}", default_config);
        }
        None => {
            println!("Configuration management commands:");
            println!("  xtrade config show    - Show current configuration");
            println!("  xtrade config set <key> <value> - Set configuration value");
            println!("  xtrade config reset   - Reset to default configuration");
        }
    }
    Ok(())
}

/// Handle demo command directly
async fn handle_demo_command() -> AppResult<()> {
    println!("Starting demo mode...");

    // Call the demo function from binance module
    if let Err(e) = xtrade::binance::demo::demo_websocket().await {
        eprintln!("Demo error: {}", e);
    }

    Ok(())
}
