use std::{io::IsTerminal, time::Duration};

use anyhow::Result;
use axum::{body::Body, extract::Request, http::Response};
use tracing::{Level, Span, Subscriber, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer, filter, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
};
use uuid::Uuid;

#[derive(clap::Args, Debug, Default)]
pub struct Instrumentation {
    /// Enable debug logs, -vv for trace
    #[clap(
    short = 'v',
    long, action = clap::ArgAction::Count,
    global = true
    )]
    pub verbose: u8,
    #[clap(
    long,
    default_value_t = Default::default(),
    global = true)]
    pub(crate) logger: Logger,
}

pub enum LogSourceFilter {
    Librq,
    Reqwest,
}

impl Instrumentation {
    pub(crate) fn level(&self) -> Level {
        match self.verbose {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    }

    pub(crate) fn log_level(&self) -> LevelFilter {
        let verbose_string = match self.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
        .to_string();

        match verbose_string.as_ref() {
            "debug" => LevelFilter::DEBUG,
            "trace" => LevelFilter::TRACE,
            _ => LevelFilter::INFO,
        }
    }

    pub(crate) fn filter_out<S>(&self, source: LogSourceFilter) -> impl Layer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        let mut filter = filter::Targets::new().with_default(self.level());
        filter = match source {
            LogSourceFilter::Librq => filter
                .with_target("librqbit_core", LevelFilter::INFO)
                .with_target("librqbit_dht", LevelFilter::WARN),
            LogSourceFilter::Reqwest => filter.with_target("reqwest", LevelFilter::INFO),
        };

        filter
    }

    pub fn setup(&self) -> Result<()> {
        let fmt_layer = match self.logger {
            Logger::Pretty => self.fmt_layer_pretty().boxed(),
            Logger::Json => self.fmt_layer_json().boxed(),
            Logger::Full => self.fmt_layer_full().boxed(),
            //TODO: change to compact when available
            Logger::Compact => self.fmt_layer_pretty().boxed(),
        };

        tracing_subscriber::registry()
            .with(self.log_level())
            .with(
                filter::Targets::new().with_default(filter::LevelFilter::from_level(self.level())),
            )
            .with(fmt_layer)
            .with(self.filter_out(LogSourceFilter::Librq))
            .try_init()?;

        Ok(())
    }
    pub(crate) fn fmt_layer_full<S>(&self) -> impl Layer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
    }

    pub(crate) fn fmt_layer_pretty<S>(&self) -> impl Layer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .pretty()
    }

    pub(crate) fn fmt_layer_json<S>(&self) -> impl Layer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .json()
    }
}

#[derive(Clone, Default, Debug, clap::ValueEnum, PartialEq, Eq)]
pub(crate) enum Logger {
    #[default]
    Compact,
    Full,
    Pretty,
    Json,
}

impl std::fmt::Display for Logger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let logger = match self {
            Logger::Compact => "compact",
            Logger::Full => "full",
            Logger::Pretty => "pretty",
            Logger::Json => "json",
        };
        write!(f, "{logger}")
    }
}

// TODO: add path parametres
pub fn trace_layer_make_span_with(request: &Request<Body>) -> Span {
    let id = Uuid::new_v4().to_string();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_else(|| &id);
    let user_agent = request
        .headers()
        .get("User-Agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
        user_agent = %user_agent,
        status = tracing::field::Empty,
        latency_us = tracing::field::Empty,
    )
}
pub fn trace_layer_on_request(_request: &Request<Body>, _span: &Span) {
    tracing::info!("Got request");
}

pub fn trace_layer_on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    let latency_ms = latency.as_millis();
    span.record("status", response.status().as_u16());
    span.record("latency_us", latency.as_micros() as u64);

    let status = response.status().as_u16();
    match status {
        500..=599 => tracing::error!("request failed with server error"),
        400..=499 => tracing::warn!("request failed with client error"),
        _ => {
            if latency_ms > 1000 {
                tracing::warn!(latency_ms = latency_ms, "slow request detected");
            } else {
                tracing::info!("request completed successfully");
            }
        }
    }
}
