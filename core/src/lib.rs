use log::{LevelFilter, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::{pattern::PatternEncoder, Encode},
    filter::threshold::ThresholdFilter,
};
use std::{backtrace, env};

pub mod result {
    pub use std::result::*;
}

pub mod area;
pub mod circular_buffer;
pub mod constants;
pub mod encrypt;
pub mod names;
pub mod ranks;
pub mod stat_buffer;
pub mod string_operations;
pub mod traits;
pub mod types;

#[derive(Debug)]
struct BacktracePatternEncoder {
    pattern_encoder: PatternEncoder,
    is_backtrace_enabled: bool,
}

impl BacktracePatternEncoder {
    fn new(pattern: &str) -> Self {
        BacktracePatternEncoder {
            pattern_encoder: PatternEncoder::new(pattern),
            is_backtrace_enabled: env::var("RUST_BACKTRACE").is_ok()
                || env::var("RUST_LIB_BACKTRACE").is_ok(),
        }
    }
}

impl Encode for BacktracePatternEncoder {
    fn encode(
        &self,
        w: &mut dyn log4rs::encode::Write,
        record: &log::Record<'_>,
    ) -> anyhow::Result<()> {
        if record.level() == log::Level::Error && self.is_backtrace_enabled {
            let args = format_args!(
                "{}\nBacktrace:\n{}",
                record.args(),
                backtrace::Backtrace::capture()
            );
            let new_record = log::Record::builder()
                .args(args)
                .level(record.level())
                .target(record.target())
                .module_path(record.module_path())
                .file(record.file())
                .line(record.line())
                .build();
            self.pattern_encoder.encode(w, &new_record)?;
        } else {
            self.pattern_encoder.encode(w, record)?;
        }
        Ok(())
    }
}

pub fn initialize_logger(
    log_level: LevelFilter,
    file_path: Option<&str>,
) -> Result<(), SetLoggerError> {
    const LOGGING_PATTERN: &'static str = "{d} {l} {f}:{L} - {m}\n";

    // Build a stderr logger - always on.
    let stderr = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
        .build();

    let mut config_builder = Config::builder();
    let mut file_appender_added = false;

    if let Some(path) = file_path {
        match FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
            .build(path)
        {
            Ok(logfile) => {
                config_builder = config_builder
                    .appender(Appender::builder().build("logfile", Box::new(logfile)));
                file_appender_added = true;
            }
            Err(e) => {
                // Cannot write to the requested log file (e.g. permission denied
                // when CWD is "/" inside a macOS .app bundle). Fall back to
                // stderr-only logging rather than panicking.
                eprintln!(
                    "Warning: could not open log file '{}': {}. Logging to stderr only.",
                    path, e
                );
            }
        }
    }

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let mut root_builder = Root::builder();
    if file_appender_added {
        root_builder = root_builder.appender("logfile");
    }
    let config = config_builder
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(root_builder.appender("stderr").build(log_level))
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;

    Ok(())
}
