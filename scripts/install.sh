#!/usr/bin/env sh
set -eu

REPO="${CODEINSIGHT_REPO:-sleticalboy/CodeInsight-mcp}"
VERSION="${CODEINSIGHT_VERSION:-latest}"

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os_target="apple-darwin" ;;
        Linux) os_target="unknown-linux-gnu" ;;
        *)
            echo "unsupported operating system: $os" >&2
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch_target="x86_64" ;;
        arm64 | aarch64) arch_target="aarch64" ;;
        *)
            echo "unsupported CPU architecture: $arch" >&2
            exit 1
            ;;
    esac

    printf "%s-%s" "$arch_target" "$os_target"
}

default_install_dir() {
    if [ -w "/usr/local/bin" ]; then
        printf "%s" "/usr/local/bin"
    else
        printf "%s" "$HOME/.local/bin"
    fi
}

download_with_gh() {
    asset="$1"
    tmp_dir="$2"

    if [ "$VERSION" = "latest" ]; then
        gh release download --repo "$REPO" --pattern "$asset" --dir "$tmp_dir"
    else
        gh release download "$VERSION" --repo "$REPO" --pattern "$asset" --dir "$tmp_dir"
    fi
}

download_with_curl() {
    asset="$1"
    tmp_dir="$2"

    if [ "$VERSION" = "latest" ]; then
        url="https://github.com/$REPO/releases/latest/download/$asset"
    else
        url="https://github.com/$REPO/releases/download/$VERSION/$asset"
    fi

    if [ -n "${GITHUB_TOKEN:-}" ]; then
        curl -fL \
            -H "Authorization: Bearer $GITHUB_TOKEN" \
            -H "Accept: application/octet-stream" \
            "$url" \
            -o "$tmp_dir/$asset"
    else
        curl -fL "$url" -o "$tmp_dir/$asset"
    fi
}

download_release_asset() {
    asset="$1"
    tmp_dir="$2"

    if command -v gh >/dev/null 2>&1; then
        if download_with_gh "$asset" "$tmp_dir"; then
            return 0
        fi

        rm -f "$tmp_dir/$asset"
        echo "gh release download failed; falling back to curl" >&2
    fi

    if command -v curl >/dev/null 2>&1; then
        download_with_curl "$asset" "$tmp_dir"
    else
        echo "install requires either gh or curl" >&2
        exit 1
    fi
}

download_local_asset() {
    asset="$1"
    tmp_dir="$2"
    local_asset="${CODEINSIGHT_ASSET_PATH:-}"

    if [ -z "$local_asset" ]; then
        return 1
    fi

    if [ ! -f "$local_asset" ]; then
        echo "local release asset not found: $local_asset" >&2
        exit 1
    fi

    cp "$local_asset" "$tmp_dir/$asset"
}

main() {
    target="${CODEINSIGHT_TARGET:-$(detect_target)}"
    asset="codeinsight-$target.tar.gz"
    install_dir="${INSTALL_DIR:-$(default_install_dir)}"
    tmp_dir="$(mktemp -d)"

    cleanup() {
        rm -rf "$tmp_dir"
    }
    trap cleanup EXIT INT TERM

    mkdir -p "$install_dir"

    if [ -n "${CODEINSIGHT_ASSET_PATH:-}" ]; then
        download_local_asset "$asset" "$tmp_dir"
    else
        download_release_asset "$asset" "$tmp_dir"
    fi

    tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
    install -m 755 "$tmp_dir/codeinsight-$target/codeinsight" "$install_dir/codeinsight"

    if [ "${CODEINSIGHT_SKIP_VERIFY:-}" != "1" ]; then
        "$install_dir/codeinsight" --help >/dev/null
    fi

    echo "codeinsight installed to $install_dir/codeinsight"
    echo "run: $install_dir/codeinsight --help"
}

main "$@"
