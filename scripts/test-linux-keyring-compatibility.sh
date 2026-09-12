#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
scratch_root=$(mktemp -d)
legacy_repository="$scratch_root/legacy"
legacy_target="$scratch_root/legacy-target"
current_target="${CARGO_TARGET_DIR:-$repository_root/target}"
legacy_seed_store="$scratch_root/legacy-seed.sqlite"
async_seed_store="$scratch_root/async-seed.sqlite"
trap 'rm -rf -- "$scratch_root"' EXIT

mkdir -p "$legacy_repository"
git -C "$repository_root" ls-files --cached --others --exclude-standard -z |
  tar -C "$repository_root" --null -T - -cf - |
  tar -C "$legacy_repository" -xf -

new_features='"async-secret-service", "async-io", "crypto-rust"'
old_features='"sync-secret-service", "crypto-rust", "vendored"'
if ! grep -Fq "$new_features" "$legacy_repository/Cargo.toml"; then
  echo "Current keyring feature selection was not found in Cargo.toml" >&2
  exit 1
fi
sed -i "s/$new_features/$old_features/" "$legacy_repository/Cargo.toml"

run_test() {
  local test_repository=$1
  local test_target=$2
  local test_store=$3
  local test_name=$4
  local deadline=$5

  (
    cd "$test_repository"
    CARGO_TARGET_DIR="$test_target" \
      POQI_KEYRING_COMPAT_STORE="$test_store" \
      timeout "$deadline" cargo test -p poqi-store --locked "$test_name" \
      -- --ignored --exact --nocapture
  )
}

(
  cd "$legacy_repository"
  timeout 2m cargo update -p keyring --precise 3.6.3
)

seed_test=tests::linux_keyring_compatibility_seed_with_selected_backend
read_write_test=tests::linux_keyring_compatibility_read_and_write_with_selected_backend
verify_test=tests::linux_keyring_compatibility_verify_with_selected_backend

run_test "$legacy_repository" "$legacy_target" "$legacy_seed_store" "$seed_test" 8m
run_test "$repository_root" "$current_target" "$legacy_seed_store" "$read_write_test" 2m
run_test "$legacy_repository" "$legacy_target" "$legacy_seed_store" "$verify_test" 2m

run_test "$repository_root" "$current_target" "$async_seed_store" "$seed_test" 2m
run_test "$legacy_repository" "$legacy_target" "$async_seed_store" "$read_write_test" 2m
run_test "$repository_root" "$current_target" "$async_seed_store" "$verify_test" 2m

echo "PASS: sync and async Secret Service backends read each other's keys and encrypted stores"
