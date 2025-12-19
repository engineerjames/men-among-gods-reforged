use log::{LevelFilter, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

pub mod constants;
pub mod types;

pub fn initialize_logger(
    log_level: LevelFilter,
    file_path: Option<&str>,
) -> Result<(), SetLoggerError> {
    // Build a stderr logger - always for now.
    let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

    let mut config_builder = Config::builder();

    if file_path.is_some() {
        let logfile = FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
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
