use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::process::Command as Cmd;

#[derive(Debug, Clone, Subcommand)]
enum Command {
    List,
    DumpInfo,
    // Show,
    // Create,
    // Edit,
    // Configure,
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

fn cmd_list(config: Config) -> Result<()> {
    let path = ensure_local_repo(config.git_repo_url, config.git_repo_checkout)?;
    let files = files_in_rfc_repo(path)?;

    files.iter().for_each(|f| println!("{}", f.display()));

    Ok(())
}

fn cmd_dump_info(config: Config) -> Result<()> {
    println!("Configuration location: {}", config_path()?.display());
    println!("git_repo_checkout path: {:?}", config.git_repo_checkout);
    println!("git_repo_url: {:?}", config.git_repo_url);
    Ok(())
}

fn files_in_rfc_repo(local_repo: PathBuf) -> Result<Vec<PathBuf>> {
    match std::fs::read_dir(local_repo) {
        Ok(entries) => Ok(filter_files_for_rfcs(entries)),
        Err(e) => {
            bail!("Error while trying to walk directory: {}", e);
        }
    }
}

fn ensure_local_repo(url: Option<String>, path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(p) => Ok(p),
        None => match url {
            Some(ref url) => {
                let config_dir = config_path()?
                    .parent()
                    .expect("Config path must have parent")
                    .to_path_buf();
                checkout_git_url_locally(config_dir, url.clone())
            }
            None => {
                bail!(
                    "No local git repo configured, and no git URL given, \
                     can't do anything.\n \
                     To configure, run `rfcs configure git-url <git URL>`, \
                     or `rfcs configure git-checkout /path/to/rfcs`."
                )
            }
        },
    }
}

fn filter_files_for_rfcs(files: std::fs::ReadDir) -> Vec<PathBuf> {
    files
        .into_iter()
        .filter_map(|dir_entry| match dir_entry {
            Ok(entry) => Some(entry.path()),
            Err(err) => {
                eprintln!("Error while processing/reading a file: {}", err);
                None
            }
        })
        .filter(|f| file_is_text_document(f))
        .filter(|f| file_has_rfc_id(f))
        .collect()
}

fn file_is_text_document(f: &Path) -> bool {
    match f.extension() {
        Some(e) => matches!(
            e.to_str().unwrap(),
            "txt" | "md" | "markdown" | "rst" | "adoc" | "org"
        ),
        None => false,
    }
}

/// We merely look for three consecutive digits in combination with the file
/// extension.
const RFC_REGEX_PATTERN: &str = r"\d{3,}";

fn file_has_rfc_id(f: &Path) -> bool {
    let re = Regex::new(RFC_REGEX_PATTERN).expect("Can't compile RFC regex");

    match f.file_name() {
        Some(name) => match name.to_str() {
            Some(name) => re.is_match(name),
            None => false,
        },
        None => false,
    }
}

fn checkout_git_url_locally(target_location: PathBuf, url: String) -> Result<PathBuf> {
    eprintln!("Cloning git repository from URL: '{}'", url);

    let mut repo = target_location.clone();
    repo.push("rfcs");

    let command_result = Cmd::new("git")
        .arg("clone")
        .arg(&url)
        .arg("rfcs")
        .current_dir(target_location)
        .output();

    match command_result {
        Ok(output) => match output.status.success() {
            true => {
                eprintln!(
                    "Successfully cloned git repository to path '{}'",
                    repo.display()
                );
                Ok(repo)
            }
            false => {
                eprintln!("Error while cloning repository from URL {}, ", url);
                eprintln!(
                    "output: \n\nStderr: {}\nStdout:{}",
                    String::from_utf8(output.stderr).unwrap(),
                    String::from_utf8(output.stdout).unwrap()
                );
                bail!("Can't proceed any further without a repository present.")
            }
        },
        Err(e) => {
            eprintln!("Error while trying to clone git repository: {}", e);
            bail!("Can't proceed any further without a repository present.")
        }
    }
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
