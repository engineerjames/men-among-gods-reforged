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

#[macro_use]
pub mod byte_operations;
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

    // Build a stderr logger - always for now.
    let stderr = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
        .build();

    let mut config_builder = Config::builder();

    if file_path.is_some() {
        let logfile = FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
            .build(file_path.unwrap())
            .unwrap();

        config_builder =
            config_builder.appender(Appender::builder().build("logfile", Box::new(logfile)));
    }

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = config_builder
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(
            Root::builder()
                .appender("logfile")
                .appender("stderr")
                .build(log_level),
        )
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;

    Ok(())
}
