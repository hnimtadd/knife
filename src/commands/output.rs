/// Simple, expandable output system
/// Separates stderr (debug) and stdout (data) cleanly

pub struct Output {
    _verbose: bool,
}

impl Output {
    pub fn new(verbose: bool) -> Self {
        Self { _verbose: verbose }
    }

    /// Print stderr messages to stderr stream
    pub fn stderr(&self, msg: &str) {
        eprintln!("{}", msg);
    }

    /// Print stdout data to stdout stream
    pub fn stdout(&self, msg: &str) {
        println!("{}", msg);
    }
}

/// Macros for println!-like usage
#[macro_export]
macro_rules! output_stderr {
    ($output:expr, $($arg:tt)*) => {
        $output.stderr(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! output_stdout {
    ($output:expr, $($arg:tt)*) => {
        $output.stdout(&format!($($arg)*));
    };
}

/// Macro to call stderr! on output (like output.stderr!("format", args))
#[macro_export]
macro_rules! stderr {
    ($output:expr, $($arg:tt)*) => {
        $output.stderr(&format!($($arg)*));
    };
}

/// Macro to call stdout! on output (like output.stdout!("format", args))
#[macro_export]
macro_rules! stdout {
    ($output:expr, $($arg:tt)*) => {
        $output.stdout(&format!($($arg)*));
    };
}
