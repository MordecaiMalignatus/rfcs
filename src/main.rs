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
    Configure { key: String, value: String },
    // Show,
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
        Command::Configure { key, value } => cmd_config(config, key, value),
    }
}

fn cmd_list(config: Config) -> Result<()> {
    let path = ensure_local_repo(config.git)?;
    let files = files_in_rfc_repo(path)?;

    files.iter().for_each(|f| println!("{}", f.display()));

    Ok(())
}

fn cmd_dump_info(config: Config) -> Result<()> {
    println!("Configuration location: {}", config_path()?.display());
    println!(
        "git.repo: {:?}",
        config.git.as_ref().and_then(|g| g.repo.as_ref())
    );
    println!(
        "git.url: {:?}",
        config.git.as_ref().and_then(|g| g.url.as_ref())
    );
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Config {
    pub git: Option<Git>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Git {
    pub repo: Option<PathBuf>,
    pub url: Option<String>,
}

fn cmd_config(mut config: Config, key: String, value: String) -> Result<()> {
    println!("Setting key {} to value {}", &key, &value);
    match key.as_str() {
        "git.url" => {
            config.git = match config.git {
                Some(git) => Some(Git {
                    url: Some(value),
                    repo: git.repo,
                }),
                None => Some(Git {
                    url: Some(value),
                    repo: None,
                }),
            }
        }
        "git.repo" => match PathBuf::try_from(value.clone()) {
            Ok(path) => {
                config.git = match config.git {
                    Some(git) => Some(Git {
                        url: git.url,
                        repo: Some(path),
                    }),
                    None => Some(Git {
                        url: None,
                        repo: Some(path),
                    }),
                }
            }
            Err(_) => {
                bail!(
                    "Was not able to convert given value '{}' into a file path, \
                       please supply a valid path.",
                    value
                )
            }
        },
        _ => {
            bail!(
                "Unknown configuration key '{}', known keys: git.url, git.repo",
                key
            )
        }
    };

    write_config(config)?;
    println!("Wrote config.");

    Ok(())
}

fn files_in_rfc_repo(local_repo: PathBuf) -> Result<Vec<PathBuf>> {
    let res = walkdir::WalkDir::new(local_repo)
        .into_iter()
        .filter_map(|dir_entry| match dir_entry {
            Ok(entry) => Some(PathBuf::from(entry.path())),
            Err(err) => {
                eprintln!("Error while processing/reading a file: {}", err);
                None
            }
        })
        .filter(|f| file_is_text_document(f))
        .filter(|f| file_has_rfc_id(f))
        .collect();

    Ok(res)
}

fn ensure_local_repo(git: Option<Git>) -> Result<PathBuf> {
    match git {
        Some(g) => match g.repo {
            Some(repo) => Ok(repo),
            None => match g.url {
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
        },
        None => bail!(
            "No local git repo configured, and no git URL given, \
             can't do anything.\n \
             To configure, run `rfcs configure git-url <git URL>`, \
             or `rfcs configure git-checkout /path/to/rfcs`."
        ),
    }
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

fn config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")?;
    Ok([&home, ".config", "rfcs", "config.toml"].iter().collect())
}

fn default_config() -> Config {
    Config { git: None }
}

fn load_config() -> Result<Config> {
    match std::fs::read_to_string(config_path()?) {
        Ok(content) => Ok(toml::from_str(&content)?),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                let config = default_config();
                write_config(config.clone())?;
                Ok(config)
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

fn write_config(config: Config) -> Result<()> {
    let p = config_path()?;
    std::fs::create_dir_all(p.parent().expect("Config path must have parent"))?;

    Ok(std::fs::write(p, toml::to_string(&config)?)?)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_positive_rfc_ids() {
        let should_match = vec![
            Path::new("./000-rfc-for-rfcs.md"),
            Path::new("./001-some-other-rfc.txt"),
            Path::new("./18215-a-future-rfc.adoc"),
        ];

        should_match
            .iter()
            .for_each(|f| assert!(file_has_rfc_id(f)));
        should_match
            .iter()
            .for_each(|f| assert!(file_is_text_document(f)));
    }

    #[test]
    fn test_negative_rfc_ids() {
        let should_not_match = vec![
            Path::new("./readme.org"),
            // TODO: There also needs to be negative extension list.
            Path::new("./91_migration.sql"),
            Path::new("./src/main.rs"),
        ];

        should_not_match
            .iter()
            .for_each(|f| assert!(!(file_has_rfc_id(f) && file_is_text_document(f))));
    }
}
