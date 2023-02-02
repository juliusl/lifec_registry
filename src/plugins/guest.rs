mod guest;
use std::time::Duration;

pub use guest::AzureGuest;

mod agent;
pub use agent::AzureAgent;

mod dispatcher;
pub use dispatcher::AzureDispatcher;

mod monitor;
use lifec::{prelude::{SpecialAttribute, ThunkContext, TimerSettings, Value}, state::AttributeIndex};
use logos::Logos;
pub use monitor::AzureMonitor;

/// Pointer-type the implements a special attribute for configuring a polling rate,
/// 
pub struct PollingRate;

/// Interprets and gets a new interval struct from a polling_rate attribute,
/// 
pub fn get_interval(tc: &ThunkContext) -> tokio::time::Interval {
    let duration = tc
        .find_float("polling_rate")
        .and_then(|f| Some(Duration::from_secs_f32(f)))
        .unwrap_or(Duration::from_millis(800));

    tokio::time::interval(duration)
}

impl SpecialAttribute for PollingRate {
    fn ident() -> &'static str {
        "polling_rate"
    }

    fn parse(parser: &mut lifec::prelude::AttributeParser, content: impl AsRef<str>) {
        match TimerSettings::lexer(content.as_ref()).next() {
            Some(TimerSettings::Duration(duration)) => {
                let entity = parser.last_child_entity().expect("should have last entity");

                parser.define_child(entity, "polling_rate", Value::Float(duration));
            }
            _ => {}
        }
    }
}
