#!/usr/bin/env sh
set -eu

REPO="poqi-cli/poqi"
BIN_NAME="poqi"

VERSION="${POQI_VERSION-latest}"
DRY_RUN="${POQI_INSTALLER_DRY_RUN-0}"

die() {
  echo "poqi installer: $*" >&2
  exit 1
}

case "$VERSION" in
  *[![:space:]]*) ;;
  *) die "POQI_VERSION must not be empty when set" ;;
esac

case "$DRY_RUN" in
  *[![:space:]]*) ;;
  *) DRY_RUN="0" ;;
esac

need() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

path_has_dir() {
  case ":$PATH:" in
    *":$1:"*) return 0 ;;
    *) return 1 ;;
  esac
}

choose_install_dir() {
  if [ "${POQI_INSTALL_DIR+x}" = "x" ]; then
    case "$POQI_INSTALL_DIR" in
      *[![:space:]]*)
        printf '%s\n' "$POQI_INSTALL_DIR"
        return
        ;;
    esac
  fi

  for dir in "$HOME/.local/bin" "$HOME/bin"; do
    if path_has_dir "$dir"; then
      printf '%s\n' "$dir"
      return
    fi
  done

  printf '%s\n' "$HOME/.local/bin"
}

shell_quote() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

profile_path() {
  shell_name="${SHELL:-}"
  shell_name="${shell_name##*/}"
  case "$shell_name" in
    zsh) printf '%s\n' "$HOME/.zshrc" ;;
    bash)
      if [ "$(uname -s)" = "Darwin" ]; then
        printf '%s\n' "$HOME/.bash_profile"
      else
        printf '%s\n' "$HOME/.bashrc"
      fi
      ;;
    *) printf '%s\n' "$HOME/.profile" ;;
  esac
}

ensure_profile_path() {
  if path_has_dir "$INSTALL_DIR"; then
    return
  fi

  profile="$(profile_path)"
  profile_dir="$(dirname "$profile")"
  quoted_dir="$(shell_quote "$INSTALL_DIR")"
  mkdir -p "$profile_dir"

  if [ -f "$profile" ] && grep -F "POQI_BIN_DIR=$quoted_dir" "$profile" >/dev/null 2>&1; then
    echo "PATH already configured in $profile"
    return
  fi

  {
    echo ""
    echo "# Added by poqi installer"
    echo "POQI_BIN_DIR=$quoted_dir"
    echo 'case ":$PATH:" in'
    echo '  *":$POQI_BIN_DIR:"*) ;;'
    echo '  *) export PATH="$POQI_BIN_DIR:$PATH" ;;'
    echo "esac"
  } >>"$profile"

  echo "Added $INSTALL_DIR to PATH in $profile"
}

need curl
need tar

INSTALL_DIR="$(choose_install_dir)"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64 | amd64) TARGET="linux-x86_64" ;;
      *) die "unsupported Linux architecture: $ARCH" ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64 | aarch64) TARGET="macos-arm64" ;;
      x86_64 | amd64) TARGET="macos-x86_64" ;;
      *) die "unsupported macOS architecture: $ARCH" ;;
    esac
    ;;
  *)
    die "unsupported OS: $OS"
    ;;
esac

if [ "$VERSION" = "latest" ]; then
  VERSION="$(
    curl --proto '=https' --tlsv1.2 -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
      sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
      head -n 1
  )"
fi

[ -n "$VERSION" ] || die "could not resolve latest release version"

ASSET="poqi-${VERSION}-${TARGET}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

if [ "$DRY_RUN" = "1" ] || [ "$DRY_RUN" = "true" ]; then
  echo "poqi installer dry run"
  echo "  version: $VERSION"
  echo "  target:  $TARGET"
  echo "  asset:   $ASSET"
  echo "  url:     $BASE_URL/$ASSET"
  echo "  dir:     $INSTALL_DIR"
  exit 0
fi

TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Installing poqi ${VERSION} for ${TARGET}"
curl --proto '=https' --tlsv1.2 -fsSL "$BASE_URL/$ASSET" -o "$TMP_DIR/$ASSET"
curl --proto '=https' --tlsv1.2 -fsSL "$BASE_URL/SHA256SUMS.txt" -o "$TMP_DIR/SHA256SUMS.txt"

EXPECTED="$(
  grep " $ASSET\$" "$TMP_DIR/SHA256SUMS.txt" |
    awk '{print $1}'
)"
[ -n "$EXPECTED" ] || die "could not find checksum for $ASSET"

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "$TMP_DIR/$ASSET" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL="$(shasum -a 256 "$TMP_DIR/$ASSET" | awk '{print $1}')"
else
  die "missing sha256sum or shasum for checksum verification"
fi

[ "$EXPECTED" = "$ACTUAL" ] || die "checksum mismatch for $ASSET"

tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"
mkdir -p "$INSTALL_DIR"
cp "$TMP_DIR/poqi-${VERSION}-${TARGET}/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"
ensure_profile_path

echo "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"
if path_has_dir "$INSTALL_DIR"; then
  echo "Run: poqi"
else
  echo "Open a new terminal, then run: poqi"
fi
