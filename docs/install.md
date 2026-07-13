# Install

CodeInsight publishes release binaries for macOS and Linux, a shared Homebrew
tap, and a GHCR Docker image. Installing from source is also supported for local
development.

## Release Installer

Install the latest release for the current macOS or Linux platform:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

Install a specific version:

```bash
CODEINSIGHT_VERSION=v0.1.11 sh scripts/install.sh
```

Choose a custom install directory:

```bash
INSTALL_DIR="$HOME/bin" sh scripts/install.sh
```

The installer supports:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

For private repositories or rate-limited environments, install and authenticate
GitHub CLI first:

```bash
gh auth login
sh scripts/install.sh
```

Without GitHub CLI, the installer falls back to `curl`. Set `GITHUB_TOKEN` if
the release assets require authentication.

Smoke test the packaged installer path locally:

```bash
scripts/release-install-smoke.sh
```

## Homebrew

Install from the shared Homebrew tap:

```bash
brew tap sleticalboy/tap
brew install codeinsight
```

## Source

Install the current checkout:

```bash
cargo install --path .
```

## Docker

Build and run the local image:

```bash
docker build -t codeinsight:local .
docker run --rm -v "$PWD:/workspace" codeinsight:local overview /workspace
```

Tagged releases publish a GHCR image:

```bash
docker pull ghcr.io/sleticalboy/codeinsight-mcp:latest
docker run --rm -v "$PWD:/workspace" ghcr.io/sleticalboy/codeinsight-mcp:latest overview /workspace
```

Release images are published for `linux/amd64` and `linux/arm64`.

Smoke test the Docker image locally:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
```

For release publishing and verification operations, see
[Release runbook](release-runbook.md).
