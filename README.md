# Gitdesk

Gitdesk is a native Git manager built with [Anomaly’s Rune UI framework](https://github.com/anomalyco/rune). Open a local repository by typing its path or choosing its folder.

## Run

With Git and the stable Rust toolchain installed, run this from the Gitdesk source directory.

```powershell
cargo run --locked
```

Pass a repository path to open it at launch, for example `cargo run --locked -- "C:\Projects\my repo"`. **Open repository…** switches to another repository or linked worktree.

## Working with a repository

Use **Changes** to stage or unstage individual files, enter a commit message, and commit the index. Gitdesk keeps a separate message draft for each repository until you close the app.

**Diff** shows tracked changes in the working tree or index. **History** shows the most recent 40 commits on the current branch. **Branches** lists local branches and lets you switch between them.

In **Diff → Compare branches**, choose a base and a comparison branch to view the changes from the base tip to the comparison tip. The pickers include local and remote-tracking branches. Use **Fetch** to update remote-tracking branches.

**Fetch**, **Pull**, and **Push** use the repository’s Git configuration and credentials. Pull uses fast-forward only. Git reports errors in the window. Bare repositories support history, branch inspection, fetch, and push.

**Settings** reads and edits Git configuration at local, global, worktree, and system scope, subject to Git’s permissions and repository configuration.

**Commands** lists installed Git commands and aliases. Enter arguments after the displayed `git` prefix to run a command in the open repository. Quote paths that contain spaces. Use **Terminal** for native shell syntax, interactive Git commands, or Git workflows involving other programs. The shell starts in the repository directory. Gitdesk refreshes repository data when you return to Changes, Diff, History, or Branches.

**GitHub CLI** runs `gh` in the open repository. With GitHub CLI installed on PATH, enter arguments such as `auth status`, `pr list`, or `run list`. Commands use your GitHub CLI configuration and can accept interactive input.
