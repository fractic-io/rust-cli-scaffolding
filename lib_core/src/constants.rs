pub const HOME: &'static str = "HOME";
pub const JAVA_HOME: &'static str = "JAVA_HOME";
pub const ANDROID_HOME: &'static str = "ANDROID_HOME";
pub const FLUTTER_HOME: &'static str = "FLUTTER_HOME";
pub const INCLUDE_IN_ENV: &'static [&'static str] = &[HOME, JAVA_HOME, ANDROID_HOME, FLUTTER_HOME];

pub const PATH: &'static str = "PATH";
pub const INCLUDE_IN_PATH: &'static [&'static str] = &[
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
    "$JAVA_HOME/bin",
    "$ANDROID_HOME/emulator",
    "$ANDROID_HOME/cmdline-tools/latest/bin",
    "$ANDROID_HOME/platform-tools",
    "$FLUTTER_HOME/bin",
    "$HOME/.cargo/bin",
    "/opt/homebrew/bin",
];
