#!/usr/bin/env bash
# Runs a command inside the KDE Flatpak SDK (Qt 6.10) with the host's Rust, for systems without Qt.
#   sdk-run.sh cargo build -p ductus-qt
# SDK_ID, SDK_BRANCH, GL_BRANCH pick the runtimes; SDK_RUN_BIND adds read-only host paths (a:b).
set -euo pipefail

SDK_ID="${SDK_ID:-org.kde.Sdk}"
SDK_BRANCH="${SDK_BRANCH:-6.10}"
GL_BRANCH="${GL_BRANCH:-25.08}"
SDK="$(flatpak info --show-location "$SDK_ID//$SDK_BRANCH")/files"
CARGO_BIN="$(dirname "$(command -v cargo)")"
RUSTUP_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
CARGO_DIR="${CARGO_HOME:-$HOME/.cargo}"
PROJECT="$(cd "$(dirname "$0")/.." && pwd)"

mkdir -p "$CARGO_DIR"

# Bind the SDK's /etc entry by entry so the host's networking files can be added.
etc_args=()
for entry in "$SDK"/etc/*; do
    name="$(basename "$entry")"
    case "$name" in resolv.conf | hosts | ssl) continue ;; esac
    if [ -L "$entry" ]; then
        etc_args+=(--symlink "$(readlink "$entry")" "/etc/$name")
    else
        etc_args+=(--ro-bind "$entry" "/etc/$name")
    fi
done

# Mesa renders without a GPU.
gl_args=()
GL_DIR=/gl
if GL="$(flatpak info --show-location "org.freedesktop.Platform.GL.default//$GL_BRANCH" 2>/dev/null)/files" \
    && [ -d "$GL" ]; then
    gl_args+=(
        --ro-bind "$GL" "$GL_DIR"
        --setenv LD_LIBRARY_PATH "$GL_DIR/lib"
        --setenv LIBGL_DRIVERS_PATH "$GL_DIR/lib/dri"
        --setenv __EGL_VENDOR_LIBRARY_DIRS "$GL_DIR/glvnd/egl_vendor.d"
        --setenv VK_DRIVER_FILES "$GL_DIR/vulkan/icd.d/lvp_icd.x86_64.json"
    )
fi

# Rust's own lld links (its gcc-ld wrapper): GNU ld.bfd, the SDK's only linker, leaves cxx-qt's
# initializers undefined. Naming it also keeps cxx-qt from looking for lld, gold or mold.
LINK_FLAGS="-Clink-arg=-fuse-ld=lld"

extra_args=()
IFS=: read -ra extra_paths <<<"${SDK_RUN_BIND:-}"
for path in "${extra_paths[@]}"; do
    extra_args+=(--ro-bind "$path" "$path")
done

exec bwrap \
    --ro-bind "$SDK" /usr \
    --symlink usr/lib /lib \
    --symlink usr/lib64 /lib64 \
    --symlink usr/bin /bin \
    --symlink usr/sbin /sbin \
    "${etc_args[@]}" \
    --ro-bind-try /etc/resolv.conf /etc/resolv.conf \
    --ro-bind-try /etc/hosts /etc/hosts \
    --ro-bind-try /etc/ssl /etc/ssl \
    --proc /proc \
    --dev /dev \
    --tmpfs /tmp \
    "${gl_args[@]}" \
    --ro-bind "$(dirname "$CARGO_BIN")" "$(dirname "$CARGO_BIN")" \
    --ro-bind "$RUSTUP_DIR" "$RUSTUP_DIR" \
    --bind "$CARGO_DIR" "$CARGO_DIR" \
    --bind "$PROJECT" "$PROJECT" \
    "${extra_args[@]}" \
    --chdir "$PROJECT" \
    --die-with-parent \
    --setenv PATH "$CARGO_DIR/bin:$CARGO_BIN:/usr/bin" \
    --setenv RUSTUP_HOME "$RUSTUP_DIR" \
    --setenv CARGO_HOME "$CARGO_DIR" \
    --setenv CARGO_TARGET_DIR "$PROJECT/target/sdk-$SDK_BRANCH" \
    --setenv RUSTFLAGS "${RUSTFLAGS:+$RUSTFLAGS }$LINK_FLAGS" \
    --setenv HOME "$HOME" \
    "$@"
