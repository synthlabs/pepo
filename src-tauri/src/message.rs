use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::{badgemanager::BadgeManager, emote::cache::EmoteCacheTrait, types::BadgeRef};

pub struct Parser {}

impl Parser {
    pub fn parse(message: String, cache: &dyn EmoteCacheTrait) -> Vec<String> {
        error!("parse - not implemented");
        Default::default()
    }
}
