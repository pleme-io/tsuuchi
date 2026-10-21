use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const FOCUS_PANE_ACTION_ID: &str = "focus_pane";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionMessage {
    FocusPane(String),
    Action(String),
}

impl ActionMessage {
    #[must_use]
    pub fn for_activation(action_id: &str, group: Option<&str>) -> Self {
        match (action_id, group) {
            (FOCUS_PANE_ACTION_ID, Some(group)) => Self::FocusPane(group.to_string()),
            _ => Self::Action(action_id.to_string()),
        }
    }

    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }

    pub fn from_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRoute {
    pub socket: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_pane_is_the_exact_documented_line() {
        let msg = ActionMessage::for_activation("focus_pane", Some("pane-7"));
        assert_eq!(msg.to_line().unwrap(), "{\"focus_pane\":\"pane-7\"}\n");
    }

    #[test]
    fn round_trips() {
        for msg in [
            ActionMessage::FocusPane("a".into()),
            ActionMessage::Action("copy".into()),
        ] {
            assert_eq!(
                ActionMessage::from_line(&msg.to_line().unwrap()).unwrap(),
                msg
            );
        }
    }

    #[test]
    fn focus_without_a_group_degrades_to_a_plain_action() {
        assert_eq!(
            ActionMessage::for_activation("focus_pane", None),
            ActionMessage::Action("focus_pane".into())
        );
    }

    #[test]
    fn unknown_lines_are_rejected() {
        assert!(ActionMessage::from_line("{\"nope\":1}").is_err());
    }
}
