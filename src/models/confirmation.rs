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
    CreateSnapshot,
    DeleteSnapshot,
    RestoreSnapshot,
    DeleteSshKey,
    ReleaseFloatingIp,
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
            "create-snapshot" => Some(Self::CreateSnapshot),
            "delete-snapshot" => Some(Self::DeleteSnapshot),
            "restore-snapshot" => Some(Self::RestoreSnapshot),
            "delete-ssh-key" => Some(Self::DeleteSshKey),
            "release-floating-ip" => Some(Self::ReleaseFloatingIp),
            _ => None,
        }
    }

    /// Convert action to string representation
    /// 
    /// Reserved for future serialization needs where we may need to convert
    /// actions back to their string form for logging or API communication.
    /// Currently, we use serde's automatic serialization.
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
            Self::CreateSnapshot => "create-snapshot",
            Self::DeleteSnapshot => "delete-snapshot",
            Self::RestoreSnapshot => "restore-snapshot",
            Self::DeleteSshKey => "delete-ssh-key",
            Self::ReleaseFloatingIp => "release-floating-ip",
        }
    }
}

