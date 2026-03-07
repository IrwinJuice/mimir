use std::io;
use time::macros::format_description;
use tracing::subscriber::set_global_default;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, prelude::*};

pub fn init() {
    let _ = std::fs::create_dir_all("./logs");

    let error_file =
        rolling::daily("./logs", "errors.bills").with_max_level(tracing::Level::ERROR);
    let info_file = rolling::daily("./logs", "bills").with_max_level(tracing::Level::INFO);
    let all_files = error_file.and(info_file);

    let local_time = tracing_subscriber::fmt::time::LocalTime::new(format_description!(
        "[day].[month].[year] [hour]:[minute]:[second].[subsecond digits:3]"
    ));

    // Tokio console layer
    // uncomment line bellow for tokio-console
    // let console_layer = console_subscriber::spawn();

    let file_layer = fmt::layer()
        .with_writer(all_files)
        .json()
        .with_timer(local_time.clone())
        .with_ansi(false);

    let console_filter = tracing_subscriber::filter::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::filter::EnvFilter::new("info"));

    let stderr_layer = fmt::layer()
        .with_writer(io::stderr)
        .with_target(true)
        .with_timer(local_time)
        .with_level(true)
        .with_filter(console_filter);

    let subscriber = tracing_subscriber::registry()
        // uncomment line bellow for tokio-console
        // .with(console_layer)
        .with(file_layer)
        .with(stderr_layer);

    set_global_default(subscriber).expect("Failed to set subscriber");
}
