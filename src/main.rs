use chrono::{NaiveDate, Utc};
use clap::Parser;
use dotenv::dotenv;
use std::fs;
use std::path::Path;

mod apis;
mod args;
mod bot;
mod cli;
mod config;
mod models;

use args::Args;
use config::Configs;

extern crate openmensa_rust_interface;
use openmensa_rust_interface::{
    get_canteen_by_id, get_canteens_by_ids, get_canteens_by_location, Canteen,
};

/// Parses a date string into a NaiveDate.
///
/// # Arguments
///
/// * `date_str` - A string slice that holds the date in format "YYYY-MM-DD" or "today".
///
/// # Returns
///
/// Returns `Ok(NaiveDate)` if parsing is successful, otherwise returns `Err(String)`.
fn parse_date(date_str: &str) -> Result<NaiveDate, String> {
    match date_str {
        "today" => Ok(Utc::now().date_naive()), // Using naive_local() for compatibility
        _ => NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|err| format!("Invalid date format: {}", err)),
    }
}

/// Fetches canteens based on the provided arguments and configurations.
///
/// # Arguments
///
/// * `args` - A reference to `Args` struct containing command-line arguments.
/// * `configs` - A reference to `Configs` struct containing configuration settings.
///
/// # Returns
///
/// Returns `Some(Vec<Canteen>)` if canteens are fetched successfully, otherwise returns `None`.
async fn fetch_canteens(args: &Args, configs: &Configs) -> Option<Vec<Canteen>> {
    if let Some(id) = args.id {
        return match get_canteen_by_id(id).await {
            Ok(Some(canteen)) => Some(vec![canteen]), // Wrap the Canteen in a Vec
            Ok(None) => {
                eprintln!("Canteen not found by ID");
                None
            }
            Err(err) => {
                eprintln!("Error fetching canteens by ID: {}", err);
                None
            }
        };
    }

    if let Some(location_str) = args.location.as_deref() {
        return match get_canteens_by_location(location_str).await {
            Ok(canteens) => Some(canteens),
            Err(err) => {
                eprintln!("Error fetching canteens by location: {}", err);
                None
            }
        };
    }

    match get_canteens_by_ids(configs.locations.canteens.to_vec()).await {
        Ok(canteens) => Some(canteens),
        Err(err) => {
            eprintln!("Error fetching canteens by IDs: {}", err);
            None
        }
    }
}

/// Reads and returns the bot token based on the provided arguments.
///
/// # Arguments
///
/// * `args` - A reference to `Args` struct containing command-line arguments.
///
/// # Returns
///
/// Returns `Ok(String)` if a token is found, otherwise returns `Err(String)`.
fn get_bot_token(args: &Args) -> Result<String, String> {
    if args.token.is_none() && args.env_file.is_none() {
        let path = Path::new(".env");
        if !path.exists() {
            return Err(
                "Please provide a Discord Token either as a parameter or in a .env file".into(),
            );
        }

        dotenv().ok();
        return std::env::var("DISCORD_TOKEN")
            .map_err(|_| "Could not find \"DISCORD_TOKEN\" in .env file".into());
    }

    if let Some(ref env_path) = args.env_file {
        let path = Path::new(env_path);
        if !path.exists() {
            return Err("Wrong path passed to arg".into());
        }

        dotenv().ok();
        return std::env::var("DISCORD_TOKEN")
            .map_err(|_| "Could not find \"DISCORD_TOKEN\" in .env file".into());
    }

    if let Some(ref arg_token) = args.token {
        return Ok(arg_token.clone());
    }

    Err("No valid token source provided".into())
}

/// Handles Discord bot functionality.
///
/// # Arguments
///
/// * `args` - A reference to `Args` struct containing command-line arguments.
///
/// # Returns
///
/// Returns `Ok(())` if the bot starts successfully, otherwise returns `Err(String)`.
async fn handle_discord_bot(args: &Args) -> Result<(), String> {
    let token = get_bot_token(args)?;
    bot::start_bot(&token).await;
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Read configuration file
    let config_path = "~/.config/discord-bot/config.toml";
    let expanded_path = shellexpand::tilde(config_path).into_owned();

    // Handle Discord bot functionality
    if args.discord_bot {
        if let Err(err) = handle_discord_bot(&args).await {
            eprintln!("{}", err);
            return;
        }
    }

    let configs_file = match fs::read_to_string(Path::new(&expanded_path)) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Error reading config file: {}", err);
            return;
        }
    };

    // Parse the TOML content
    let configs: Configs = match toml::from_str(&configs_file) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("Failed to parse the TOML: {}", err);
            return;
        }
    };

    // Handle CLI commands or print meals for canteens
    if args.meme {
        cli::meme().await;
        return;
    }

    if args.daily_fact {
        cli::daily_fact().await;
        return;
    }

    if args.random_fact {
        cli::random_fact().await;
        return;
    }

    if args.id.is_some() && args.location.is_some() {
        eprintln!("Use either location or id");
        return;
    }

    // Fetch and print meals for canteens
    if let Some(canteens) = fetch_canteens(&args, &configs).await {
        let date = match parse_date(&args.date) {
            Ok(date) => date,
            Err(err) => {
                eprintln!("Error parsing date: {}", err);
                return;
            }
        };

        if let Err(err) = cli::print_meals(canteens, date).await {
            eprintln!("Error printing meals: {}", err);
        }
    }
}
