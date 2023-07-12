use std::path::PathBuf;

use anyhow::bail;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Subcommand)]
enum Command {
    List,
    DumpInfo, // Show,
              // Create,
              // Edit,
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = load_config()?;
    match args.command {
        Command::List => cmd_list(config),
        Command::DumpInfo => cmd_dump_info(config),
    }
}
fn cmd_list(_config: Config) -> Result<()> {
    todo!()
}

fn cmd_dump_info(config: Config) -> Result<()> {
    println!("Configuration location: {}", config_path()?.display());
    println!("git_repo_checkout path: {:?}", config.git_repo_checkout);
    println!("git_repo_url: {:?}", config.git_repo_url);
    Ok(())
}

fn files_in_rfc_repo(config: Config) -> Result<Vec<PathBuf>> {
    match config.git_repo_checkout {
        Some(p) => {}
        None => match config.git_repo_url {
            Some(url) => {
                _checkout_git_url_locally(config)?;
                files_in_rfc_repo(config)
            }
            None => {
                eprintln!(
                    "No local git repo configured, and no git URL given, \
                           can't do anything."
                );
                eprintln!(
                    "To configure, run `rfcs configure git-url <git URL>`, \
                          or `rfcs configure git-checkout /path/to/rfcs`."
                );
                bail!("Can't do anything.")
            }
        },
    }
}

fn _checkout_git_url_locally(config: Config) -> Result<PathBuf> {
    todo!()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Config {
    pub git_repo_checkout: Option<PathBuf>,
    pub git_repo_url: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")?;
    Ok([&home, ".config", "rfcs", "config.toml"].iter().collect())
}

fn default_config() -> Config {
    Config {
        git_repo_checkout: None,
        git_repo_url: None,
    }
}

fn load_config() -> Result<Config> {
    match std::fs::read_to_string(config_path()?) {
        Ok(content) => Ok(toml::from_str(&content)?),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                write_default_config()?;
                Ok(default_config())
            }
            _ => {
                let context = format!(
                    "Unexpected error when reading config file from {}",
                    config_path()?.display()
                );
                Err(anyhow::Error::new(e).context(context))
            }
        },
    }
}

fn write_default_config() -> Result<()> {
    let p = config_path()?;
    std::fs::create_dir_all(p.parent().expect("Config path must have parent"))?;
    let config = default_config();

    Ok(std::fs::write(
        p.file_name()
            .expect("Fixed config path must have file name"),
        toml::to_string(&config)?,
    )?)
}
