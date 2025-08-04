use crate::common::logging::Instrumentation;

#[derive(clap::Parser, Debug)]
pub struct InputArgs {
    pub bind: Option<String>,
    #[clap(flatten)]
    pub instrumentation: Instrumentation,
}
