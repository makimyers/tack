#!/bin/sh
# Check the locked Git sources and installed Bazzite runtimes, or fetch missing worktrees.
set -eu

# Resolve this when invoked via Tack's installed checker link.
here=$(dirname "$(readlink -f "$0")")
. "$here/versions.env"
projects=${TACK_PROJECTS_DIR:-$HOME/Projects}
mode=${1:-check}
choice=${2:-}
avalon_revision=
staging=
result=0

trap 'if [ -n "$staging" ]; then rm -rf -- "$staging"; fi' EXIT
trap 'exit 1' HUP INT TERM

case "$mode" in
  check|sync) ;;
  weights) ;;
  *) echo "Usage: $0 {check|sync} [repo] or $0 weights" >&2; exit 2 ;;
esac

report() {
  printf '%s\t%s\t%s\n' "$1" "$2" "$3"
  if [ "$1" = fail ]; then result=1; fi
}

# Take content equality for patched sources, since the reconstructed commit has a new ID.
check_repo() {
  if [ ! -e "$dir/.git" ]; then
    report fail "$name" "missing Git checkout at $dir"
  elif ! changes=$(git -C "$dir" status --porcelain --untracked-files=no); then
    report fail "$name" "cannot read Git checkout at $dir"
  elif [ -n "$changes" ]; then
    report fail "$name" "tracked files have local changes in $dir"
  elif [ "$tree" != - ]; then
    if [ "$(git -C "$dir" rev-parse 'HEAD^{tree}')" = "$tree" ]; then
      report ok "$name" "locked source tree $tree"
    else
      report fail "$name" "source tree differs from $tree"
    fi
  elif [ "$(git -C "$dir" rev-parse HEAD)" = "$revision" ]; then
    report ok "$name" "$revision"
  else
    report fail "$name" "checkout differs from $revision"
  fi
}

fetch_repo() {
  mkdir -p "$projects"
  staging=$(mktemp -d "$projects/.$name.XXXXXX")
  git -C "$staging" init -q
  git -C "$staging" remote add origin "$url"
  git -C "$staging" fetch -q --no-tags --depth=1 origin "$revision"
  test "$(git -C "$staging" rev-parse FETCH_HEAD)" = "$revision"
  git -C "$staging" checkout -q --detach FETCH_HEAD
  if [ "$patch" != - ]; then
    git -C "$staging" apply --index "$here/$patch"
    test "$(git -C "$staging" write-tree)" = "$tree"
    git -C "$staging" -c user.name=Tack -c user.email=tack@localhost \
      -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm "Apply locked Tack changes"
  fi
  # GNU mv leaves the staging directory in place if another process claimed the destination.
  mv -T -n -- "$staging" "$dir"
  if [ -d "$staging" ]; then
    report fail "$name" "destination appeared during fetch: $dir"
    exit 1
  fi
  staging=
}

check_weights() {
  model_dir=${AVALON_MODEL_DIR:-$HOME/Avalon-models/Qwen3.8-Flash-Next-Avalon-GGUF}
  problem=
  while read -r bytes hash file; do
    if [ "$(stat -Lc %s -- "$model_dir/$file" 2>/dev/null || true)" != "$bytes" ]; then
      problem="$file is missing or has the wrong length"
      break
    fi
    if [ "$mode" = weights ]; then
      if ! printf '%s  %s\n' "$hash" "$model_dir/$file" | sha256sum --status -c -; then
        problem="$file failed SHA-256 verification"
        break
      fi
    fi
  done < "$here/avalon-weights.tsv"
  if [ -n "$problem" ]; then
    report fail avalon-weights "$problem"
  else
    if [ "$mode" = weights ]; then
      report ok avalon-weights "all locked files passed SHA-256 verification"
    else
      report ok avalon-weights "all locked files match their sizes"
    fi
  fi
}

check_t3_runtime() {
  t3_tree=$(awk '$1 == "fork-t3code" { print $4 }' "$here/dependencies.lock")
  t3_launcher="$HOME/.local/bin/t3"
  t3_runtime="${T3CODE_HOME:-$HOME/.t3}/runtime/versions/$T3_VERSION"
  launcher_path=$(readlink -f "$t3_launcher" 2>/dev/null || true)
  launcher_version=
  case "$launcher_path" in
    */t3/bin/t3.mjs)
      launcher_version=$(node -p 'require(process.argv[1]).version' "${launcher_path%/bin/*}/package.json" 2>/dev/null || true)
      ;;
  esac
  if [ -n "$t3_tree" ] && [ -x "$t3_runtime/t3" ] && [ "$(cat "$t3_runtime/.source-tree" 2>/dev/null || true)" = "$t3_tree" ] &&
     [ "$(cat "$t3_runtime/.install-complete" 2>/dev/null || true)" = "$T3_VERSION" ] &&
     [ "$launcher_version" = "$T3_VERSION" ] && [ "$("$t3_runtime/t3" --version 2>/dev/null || true)" = "t3 v$T3_VERSION" ]; then
    report ok t3-runtime "$T3_VERSION, locked source tree"
  else
    report fail t3-runtime "expected $T3_VERSION built from the locked sources; run t3-fork.sh"
  fi
}

if [ "$mode" = check ] && [ "$choice" = t3-runtime ]; then
  check_t3_runtime
  exit "$result"
fi

if [ "$mode" = weights ]; then
  check_weights
  exit "$result"
fi

found=false
while read -r name url revision tree patch; do
  case "$name" in ""|\#*) continue ;; esac
  if [ -n "$choice" ] && [ "$choice" != "$name" ]; then continue; fi
  found=true
  dir="$projects/$name"
  if [ "$name" = fork-avalon ]; then avalon_revision=$revision; fi
  if [ "$mode" = sync ] && [ ! -e "$dir" ] && [ ! -L "$dir" ]; then
    fetch_repo
  fi
  check_repo
done < "$here/dependencies.lock"

if [ "$found" = false ]; then echo "No locked repo named $choice" >&2; exit 2; fi
if [ "$mode" = sync ] || [ -n "$choice" ]; then exit "$result"; fi

check_t3_runtime

pi_version=$("$HOME/.local/bin/pi" --version 2>/dev/null || true)
if [ "$pi_version" = "$PI_VERSION" ]; then
  report ok pi-runtime "$pi_version"
else
  report fail pi-runtime "expected Pi $PI_VERSION, found ${pi_version:-no executable}"
fi

rocm="$projects/fork-strata/.venv/lib/python3.14/site-packages"
avalon_version=$(LD_LIBRARY_PATH="$rocm/_rocm_sdk_devel/lib:$rocm/_rocm_sdk_libraries_gfx110X_dgpu/lib" \
  "$projects/fork-avalon/build/bin/llama-server" --version 2>&1 || true)
avalon_build=$(printf '%s\n' "$avalon_version" | sed -n 's/.*commit \([0-9a-f][0-9a-f]*\)).*/\1/p')
# Launcher and documentation commits do not require rebuilding the engine.
if [ -n "$avalon_build" ] && git -C "$projects/fork-avalon" diff --quiet "$avalon_build" "$avalon_revision" -- . ':!local' 2>/dev/null; then
  report ok avalon-runtime "build $avalon_build loads ROCm and matches the locked engine sources"
else
  report fail avalon-runtime "check the ROCm SDK and run fork-avalon/local/build.sh"
fi

check_weights
exit "$result"
