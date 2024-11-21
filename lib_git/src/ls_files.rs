use git2::{Repository, StatusOptions};
use lib_core::{cp, define_cli_error, mkdir_p, rm_rf, CliError, CriticalError, Printer};
use std::{
    fs,
    path::{Display, Path, PathBuf},
};

define_cli_error!(NotAGitRepository, "The given path is not a git repository.");
define_cli_error!(
    FailedToFetchStatuses,
    "Failed to fetch statuses from the git repository."
);
define_cli_error!(
    FailedToFetchSubmodules,
    "Failed to fetch submodules from the git repository."
);
define_cli_error!(
    FileCantBeCanonicalized,
    "The file path '{file_path}' cannot be canonicalized. Most likely it does not exist.",
    { file_path: &Display<'_> }
);

pub fn find_git_root<P: AsRef<Path>>(discover_path: P) -> Result<PathBuf, CliError> {
    Repository::discover(discover_path)
        .map_err(|e| NotAGitRepository::with_debug(&e))?
        .workdir()
        .map_or_else(
            || Err(NotAGitRepository::new()),
            |workdir| Ok(workdir.to_path_buf()),
        )
}

pub fn list_git_tracked_files<P: AsRef<Path>>(
    repo_discover_path: P,
    filter_subpath: Option<&str>,
    include_submodules: bool,
) -> Result<Vec<PathBuf>, CliError> {
    let mut tracked_files = Vec::new();

    // Open the repository at the given path.
    let repo_root = {
        let r = find_git_root(repo_discover_path.as_ref())?;
        fs::canonicalize(&r).map_err(|e| FileCantBeCanonicalized::with_debug(&r.display(), &e))?
    };
    let filter_subpath = filter_subpath
        .map(|subpath| {
            let s = repo_root.join(subpath);
            fs::canonicalize(&s).map_err(|e| FileCantBeCanonicalized::with_debug(&s.display(), &e))
        })
        .transpose()?;
    let repo = Repository::open(&repo_root).map_err(|e| NotAGitRepository::with_debug(&e))?;

    // Create StatusOptions to list all tracked files.
    let mut status_opts = StatusOptions::new();
    status_opts
        .include_ignored(false)
        .include_untracked(false)
        .include_unmodified(true)
        .exclude_submodules(true);

    // Get the repository's status entries.
    let statuses = repo
        .statuses(Some(&mut status_opts))
        .map_err(|e| FailedToFetchStatuses::with_debug(&e))?;
    for entry in statuses.iter() {
        if let Some(path_str) = entry.path() {
            let path = repo_root.join(path_str);
            let include = match filter_subpath {
                Some(ref subpath) => path.starts_with(subpath),
                None => true,
            };
            if include {
                tracked_files.push(path);
            }
        }
    }

    if include_submodules {
        // Handle submodules.
        for submodule in repo
            .submodules()
            .map_err(|e| FailedToFetchSubmodules::with_debug(&e))?
        {
            let submodule_path = {
                let s = repo_root.join(submodule.path());
                fs::canonicalize(&s)
                    .map_err(|e| FileCantBeCanonicalized::with_debug(&s.display(), &e))?
            };
            let include = match filter_subpath {
                Some(ref subpath) => submodule_path.starts_with(subpath),
                None => true,
            };
            if include {
                if let Ok(_sub_repo) = submodule.open() {
                    let submodule_files =
                        list_git_tracked_files(submodule_path, None, include_submodules)?;
                    tracked_files.extend(submodule_files);
                }
            }
        }
    }

    Ok(tracked_files)
}

pub fn clone_repo_to<P: AsRef<Path>, Q: AsRef<Path>>(
    pr: &Printer,
    repo_discover_path: P,
    filter_subpath: Option<&str>,
    destination: Q,
    include_submodules: bool,
) -> Result<(), CliError> {
    let repo_root = find_git_root(repo_discover_path.as_ref())?;
    pr.info(&match filter_subpath {
        Some(subpath) => format!(
            "Cloning repo at {:?} to {:?} (subpath: {:?})...",
            repo_root,
            destination.as_ref(),
            subpath
        ),
        None => format!(
            "Cloning repo at {:?} to {:?}...",
            repo_root,
            destination.as_ref()
        ),
    });
    let tracked_files =
        list_git_tracked_files(&repo_discover_path, filter_subpath, include_submodules)?;
    for file in tracked_files {
        let relative_path = file.strip_prefix(&repo_root).map_err(|e| {
            CriticalError::with_debug(
                "file returned by list_git_tracked_files(...) was not inside the repo root",
                &e,
            )
        })?;
        let destination_path = destination.as_ref().join(relative_path);
        mkdir_p(destination_path.parent().ok_or_else(|| {
            CriticalError::new(
                "file returned by list_git_tracked_files(...) does not have a valid parent dir",
            )
        })?)?;
        let result = cp(&file, &destination_path);
        if let Err(_) = result {
            pr.warn(&format!(
                "File {:?} not copied.",
                file.strip_prefix(&repo_root).unwrap()
            ));
        }
    }
    Ok(())
}

pub fn with_repo_temporarily_cloned_to<P: AsRef<Path>, Q: AsRef<Path>, F, R>(
    pr: &Printer,
    repo_discover_path: P,
    filter_subpath: Option<&str>,
    destination: Q,
    include_submodules: bool,
    f: F,
) -> Result<R, CliError>
where
    F: FnOnce(&Path) -> Result<R, CliError>,
{
    pr.info("Temporarily cloning repository...");
    clone_repo_to(
        pr,
        repo_discover_path,
        filter_subpath,
        &destination,
        include_submodules,
    )?;
    let result = f(destination.as_ref());
    pr.info("Cleaning up temporary clone...");
    rm_rf(&destination)?;
    result
}
