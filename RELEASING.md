# Publishing an update

Players get updates automatically: when the game starts it asks GitHub for the latest release of
your repository and, if the version is newer than the one they are running, shows an
"Update available" box on the title screen (and in Settings > Display > Check for updates).
They click **Download update**, then **Restart now**.

## One-time setup

1. Create a **public** GitHub repository and push this folder (`Cargo.toml`, `src/`, `.github/`).
   `target/` and the settings/save files are ignored by `.gitignore`.
2. Tell the game which repository to check, in `src/update.rs`:
   ```rust
   pub const DEFAULT_REPO: &str = "your-username/your-repo";
   ```
   (Players can also override it by putting `update_repo=owner/name` in `island_settings.txt`.)
3. Build and hand out the first `Island.exe` to your players (put it in a release too).

## Releasing a new version

1. Bump `version` in `Cargo.toml`, for example `0.5.0` to `0.5.1`.
   Bump `VERSION` in `src/net.rs` too if multiplayer or the world changed, so players on old
   builds get a clear "different version" message instead of a broken game.
2. Commit and push, then tag and push the tag:
   ```
   git tag v0.5.1
   git push origin v0.5.1
   ```
3. The **Release** workflow builds `Island.exe`, writes `Island.exe.sha256`, and attaches both to a
   new GitHub release. The game verifies the checksum before installing.

## How the update works

- The game runs `curl.exe` (built into Windows 10/11) against
  `https://api.github.com/repos/<repo>/releases/latest`; no tokens are needed for public repos.
- The new exe is downloaded next to the old one as `Island.exe.new`, checked, then the running
  exe is renamed to `Island.exe.old`, the new one takes its place, and the game relaunches.
  The `.old` file is deleted on the next start. If anything fails the old version is kept.
- Private repositories: set the `ISLAND_GITHUB_TOKEN` environment variable on the player's PC.
