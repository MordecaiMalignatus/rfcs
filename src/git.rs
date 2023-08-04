use std::path::Path;

use anyhow::{bail, Context, Result};
use git2::{build::CheckoutBuilder, ErrorCode};

/// Retrieve all git branches in `path`, and strip them down to just their name.
/// Implicitly requires that the repository under `path` be a git repository,
/// but so does the rest of the program.
pub fn list_branches(path: &Path) -> Result<Vec<String>> {
    let res = init_repo(path)?;
    let branches = res
        .branches(Some(git2::BranchType::Local))
        .with_context(|| {
            format!(
                "Failed listing local branches from git repository at {}",
                path.display()
            )
        })?
        .filter_map(|r| match r {
            Ok((branch, _)) => Some(
                branch
                    .name()
                    .expect("Error while reading branch name.")
                    .map(|s| s.to_string()),
            ),
            Err(e) => {
                eprintln!(
                    "Error while listing branch at repo {}: {}",
                    path.display(),
                    e
                );
                None
            }
        })
        .map(|branch_name| branch_name.unwrap_or(String::from("<invalid utf-8 branch name>")))
        .collect();

    Ok(branches)
}

/// Works like `git checkout -b branch_name`, in that it first creates the
/// branch, then updates HEAD to track that branch.
pub fn create_and_switch_to_branch(path: &Path, branch_name: &str) -> Result<()> {
    let repo = init_repo(path)?;
    let current_main_head = find_main_branch_head(&repo)?
        .peel_to_commit()
        .context("Can't peel main head reference to commit")?;
    let branch = repo.branch(branch_name, &current_main_head, false)?;

    // Checking out a branch is a multi-step process: First we need to check out
    // the tree associated with the branch we just created,
    match repo.checkout_tree(
        current_main_head.as_object(),
        Some(CheckoutBuilder::new().safe()),
    ) {
        Ok(()) => {},
        Err(e) => {
            bail!("Error while checking out tree: {}", e)
        }
    };

    // Then we need to update HEAD to make git reflect those changes, and update
    // it to the new branch.
    repo.set_head_bytes(
        repo.resolve_reference_from_short_name(
            branch
                .name()
                .expect("We expect a branch we just created to be there.")
                .unwrap(),
        )?
        .name_bytes(),
    )?;

    Ok(())
}

/// Finds the current commit associated with either of the branches `main` or
/// `master`, with preference given to `main`.
fn find_main_branch_head(repo: &'_ git2::Repository) -> Result<git2::Reference<'_>> {
    let reference = match repo.find_branch("main", git2::BranchType::Local) {
        Ok(branch) => branch,
        Err(e) => {
            if e.code() == ErrorCode::Exists {
                repo.find_branch("master", git2::BranchType::Local)
                    .with_context(|| {
                        "Neither 'main' nor 'master' are valid \
                         references in the specified git repo."
                    })?
            } else {
                bail!("Unexpected git error: {}", e)
            }
        }
    }
    .into_reference();

    Ok(reference)
}

fn init_repo(path: &Path) -> Result<git2::Repository> {
    let d = path.display();
    git2::Repository::init(path).with_context(|| format!("Failed to open git repository at {}", d))
}
