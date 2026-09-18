use log::{LevelFilter, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
        rolling_file::{
            RollingFileAppender,
            policy::compound::{
                CompoundPolicy, roll::fixed_window::FixedWindowRollerBuilder,
                trigger::size::SizeTrigger,
            },
        },
    },
    config::{Appender, Config, Logger, Root},
    encode::{Encode, pattern::PatternEncoder},
    filter::threshold::ThresholdFilter,
};
use std::{backtrace, env};

/// Maximum size of an active rotating log file before it is archived.
pub const ROTATING_LOG_MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Number of archived files retained beside an active rotating log.
pub const ROTATING_LOG_BACKUPS: u32 = 5;

const LOGGING_PATTERN: &str = "{d(%Y-%m-%dT%H:%M:%S%.f)(utc)} {l} {f}:{L} - {m}\n";

pub mod result {
    pub use std::result::*;
}

pub mod area;
pub mod ban_action_store;
pub mod ban_store;
pub mod character_store;
pub mod circular_buffer;
pub mod client_commands;
pub mod constants;
pub mod item_store;
pub mod logout_reasons;
pub mod map_store;
pub mod marble;
pub mod monster_classes;
pub mod names;
pub mod performance_wrapper;
pub mod ranks;
pub mod server_commands;
pub mod seyan_runes;
pub mod skills;
pub mod stat_buffer;
pub mod string_operations;
pub mod talent_trees;
pub mod template_store;
pub mod text_store;
pub mod traits;
pub mod types;
pub mod weather;
pub mod weather_areas;
pub mod world_action_store;

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

/// Initializes the global logger with stderr output and an optional log file.
///
/// Stderr always receives messages at `log_level` or above. When
/// `file_path` is provided and writable, log output is also written
/// to that file. If the file cannot be opened, logging silently
/// falls back to stderr only.
///
/// # Arguments
///
/// * `log_level` - Minimum severity that reaches stderr.
/// * `file_path` - Optional path to a log file.
///
/// # Returns
///
/// * `Ok(())` on success, or a `SetLoggerError` if a logger was already set.
pub fn initialize_logger(
    log_level: LevelFilter,
    file_path: Option<&str>,
    perf_file_path: Option<&str>,
) -> Result<(), SetLoggerError> {
    // Build a stderr logger - always on.
    let stderr = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
        .build();

    let mut config_builder = Config::builder();
    let mut root_builder = Root::builder().appender("stderr");
    if let Some(path) = file_path {
        match FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
            .build(path)
        {
            Ok(logfile) => {
                config_builder = config_builder
                    .appender(Appender::builder().build("logfile", Box::new(logfile)));
                root_builder = root_builder.appender("logfile");
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

    // Perf logging
    if let Some(path) = perf_file_path
        && let Ok(perf_file) = FileAppender::builder()
            .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
            .build(path)
    {
        config_builder =
            config_builder.appender(Appender::builder().build("perf_file", Box::new(perf_file)));

        // Route target "perf" to perf_file.
        // additive(false): only perf_file
        // additive(true): perf_file + root (stderr/logfile)
        config_builder = config_builder.logger(
            Logger::builder()
                .appender("perf_file")
                .additive(false)
                .build("perf", LevelFilter::Info),
        );
    }

    let config = config_builder
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(root_builder.build(log_level))
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;

    Ok(())
}

fn rolling_file_appender(
    path: &str,
    encoder: BacktracePatternEncoder,
) -> anyhow::Result<RollingFileAppender> {
    let roller =
        FixedWindowRollerBuilder::default().build(&format!("{path}.{{}}"), ROTATING_LOG_BACKUPS)?;
    let policy = CompoundPolicy::new(
        Box::new(SizeTrigger::new(ROTATING_LOG_MAX_BYTES)),
        Box::new(roller),
    );

    Ok(RollingFileAppender::builder()
        .encoder(Box::new(encoder))
        .build(path, Box::new(policy))?)
}

/// Initializes the global logger with fixed-window rotating file output.
///
/// Stderr always receives messages at `log_level`. Each configured file keeps
/// its active file plus [`ROTATING_LOG_BACKUPS`] archived files, rotating when
/// the active file exceeds [`ROTATING_LOG_MAX_BYTES`]. The `perf` target is
/// routed only to its dedicated file, matching [`initialize_logger`].
///
/// # Arguments
///
/// * `log_level` - Minimum severity that reaches stderr and the main file.
/// * `file_path` - Optional path to the rotating main log file.
/// * `perf_file_path` - Optional path to the rotating performance log file.
///
/// # Returns
///
/// * `Ok(())` on success, or an error if logger construction or installation fails.
pub fn initialize_rotating_logger(
    log_level: LevelFilter,
    file_path: Option<&str>,
    perf_file_path: Option<&str>,
) -> anyhow::Result<()> {
    let stderr = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(BacktracePatternEncoder::new(LOGGING_PATTERN)))
        .build();

    let mut config_builder = Config::builder();
    let mut root_builder = Root::builder().appender("stderr");

    if let Some(path) = file_path {
        match rolling_file_appender(path, BacktracePatternEncoder::new(LOGGING_PATTERN)) {
            Ok(logfile) => {
                config_builder = config_builder
                    .appender(Appender::builder().build("logfile", Box::new(logfile)));
                root_builder = root_builder.appender("logfile");
            }
            Err(error) => {
                eprintln!(
                    "Warning: could not open rotating log file '{}': {}. Logging to stderr only.",
                    path, error
                );
            }
        }
    }

    if let Some(path) = perf_file_path {
        match rolling_file_appender(path, BacktracePatternEncoder::new(LOGGING_PATTERN)) {
            Ok(perf_file) => {
                config_builder = config_builder
                    .appender(Appender::builder().build("perf_file", Box::new(perf_file)));
                config_builder = config_builder.logger(
                    Logger::builder()
                        .appender("perf_file")
                        .additive(false)
                        .build("perf", LevelFilter::Info),
                );
            }
            Err(error) => {
                eprintln!(
                    "Warning: could not open rotating perf log file '{}': {}. Perf logging disabled.",
                    path, error
                );
            }
        }
    }

    let config = config_builder
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(root_builder.build(log_level))?;

    log4rs::init_config(config)?;
    Ok(())
}
