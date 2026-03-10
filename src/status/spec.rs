#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusSpec {
    pub animated: bool,
    pub escalates_to_page_overlay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSpec {
    pub drawer_open_ms: u32,
    pub welcome_transition_ms: u32,
}

pub fn status_spec(state: ConnectionState) -> StatusSpec {
    match state {
        ConnectionState::Connecting => StatusSpec {
            animated: true,
            escalates_to_page_overlay: false,
        },
        ConnectionState::Connected => StatusSpec {
            animated: false,
            escalates_to_page_overlay: false,
        },
        ConnectionState::Disconnected | ConnectionState::Error => StatusSpec {
            animated: false,
            escalates_to_page_overlay: false,
        },
    }
}

pub fn motion_spec() -> MotionSpec {
    MotionSpec {
        drawer_open_ms: 220,
        welcome_transition_ms: 160,
    }
}
