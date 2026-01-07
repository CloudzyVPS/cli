use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfirmationAction {
    DeleteUser,
    DeleteInstance,
    PowerOnInstance,
    PowerOffInstance,
    ResetInstance,
    SwitchVersion,
    ChangeOs,
    ResizeInstance,
    AddTraffic,
}

impl ConfirmationAction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "delete-user" => Some(Self::DeleteUser),
            "delete-instance" => Some(Self::DeleteInstance),
            "power-on-instance" => Some(Self::PowerOnInstance),
            "power-off-instance" => Some(Self::PowerOffInstance),
            "reset-instance" => Some(Self::ResetInstance),
            "switch-version" => Some(Self::SwitchVersion),
            "change-os" => Some(Self::ChangeOs),
            "resize-instance" => Some(Self::ResizeInstance),
            "add-traffic" => Some(Self::AddTraffic),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::DeleteUser => "delete-user",
            Self::DeleteInstance => "delete-instance",
            Self::PowerOnInstance => "power-on-instance",
            Self::PowerOffInstance => "power-off-instance",
            Self::ResetInstance => "reset-instance",
            Self::SwitchVersion => "switch-version",
            Self::ChangeOs => "change-os",
            Self::ResizeInstance => "resize-instance",
            Self::AddTraffic => "add-traffic",
        }
    }
}

