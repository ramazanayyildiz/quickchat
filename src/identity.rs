//! Product identity. ZapFast by default; the `quickchat` feature builds
//! QuickChat, a separate macOS companion that must never share ZapFast's
//! folders, keyring entries, instance slot, or update channel.

/// Whether this build is QuickChat.
pub const QUICKCHAT: bool = cfg!(feature = "quickchat");

/// The name people see.
pub const NAME: &str = if QUICKCHAT { "QuickChat" } else { "ZapFast" };

/// The command-line name, and the name `--version` reports.
pub const BIN: &str = if QUICKCHAT { "quickchat" } else { "zapfast" };

/// The per-user folder name under the platform's config, state, and cache.
pub const DIRS: &str = if QUICKCHAT { "quickchat" } else { "zapfast" };

/// The keyring service that holds each archive's key.
pub const KEYRING_SERVICE: &str = if QUICKCHAT {
    "app.quickchat.archive"
} else {
    "rocks.zapfast.ZapFast"
};

/// The single-instance slot. ZapFast keeps FastsApp's name for older copies.
pub const INSTANCE: &str = if QUICKCHAT { "quickchat" } else { "fastsapp" };

/// The macOS login item label.
pub const LAUNCH_AGENT: &str = if QUICKCHAT {
    "io.github.ramazanayyildiz.quickchat"
} else {
    "me.paolino.zapfast"
};
