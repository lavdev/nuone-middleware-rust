use std::time::SystemTime;

/// Domain state for an interactive ERP session.
///
/// The TCP socket is deliberately held by an outbound adapter, not by this model.
#[derive(Clone, Debug)]
pub struct Session {
    session_id: String,
    created_at: SystemTime,
    username: Option<String>,
    application: Option<String>,
    application_access_name: Option<String>,
    vm: Option<String>,
    server_host: Option<String>,
    authenticated: bool,
    initialized: bool,
    focus_control_id: Option<String>,
    focus_parent_id: Option<String>,
    persistent_agent_active: bool,
    nuone_service_mode: bool,
}

impl Session {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            created_at: SystemTime::now(),
            username: None,
            application: None,
            application_access_name: None,
            vm: None,
            server_host: None,
            authenticated: false,
            initialized: false,
            focus_control_id: None,
            focus_parent_id: None,
            persistent_agent_active: false,
            nuone_service_mode: false,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn set_username(&mut self, value: Option<String>) {
        self.username = value;
    }

    pub fn application(&self) -> Option<&str> {
        self.application.as_deref()
    }

    pub fn set_application(&mut self, value: Option<String>) {
        self.application = value;
    }

    pub fn application_access_name(&self) -> Option<&str> {
        self.application_access_name.as_deref()
    }

    pub fn set_application_access_name(&mut self, value: Option<String>) {
        self.application_access_name = value;
    }

    pub fn vm(&self) -> Option<&str> {
        self.vm.as_deref()
    }

    pub fn set_vm(&mut self, value: Option<String>) {
        self.vm = value;
    }

    pub fn server_host(&self) -> Option<&str> {
        self.server_host.as_deref()
    }

    pub fn set_server_host(&mut self, value: Option<String>) {
        self.server_host = value;
    }

    pub fn authenticated(&self) -> bool {
        self.authenticated
    }

    pub fn mark_authenticated(&mut self) {
        self.authenticated = true;
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    pub fn update_focus(&mut self, control_id: Option<String>, parent_id: Option<String>) {
        self.focus_control_id = control_id;
        self.focus_parent_id = parent_id;
    }

    pub fn focus_control_id(&self) -> Option<&str> {
        self.focus_control_id.as_deref()
    }

    pub fn focus_parent_id(&self) -> Option<&str> {
        self.focus_parent_id.as_deref()
    }

    pub fn persistent_agent_active(&self) -> bool {
        self.persistent_agent_active
    }

    pub fn set_persistent_agent_active(&mut self, active: bool) {
        self.persistent_agent_active = active;
    }

    pub fn nuone_service_mode(&self) -> bool {
        self.nuone_service_mode
    }

    pub fn set_nuone_service_mode(&mut self, active: bool) {
        self.nuone_service_mode = active;
    }
}

#[cfg(test)]
mod tests {
    use super::Session;

    #[test]
    fn tracks_authentication_initialization_focus_and_keepalive_state() {
        let mut session = Session::new("session-42");

        assert_eq!(session.session_id(), "session-42");
        assert!(!session.authenticated());
        assert!(!session.initialized());
        assert!(!session.persistent_agent_active());

        session.set_username(Some("erp-user".to_owned()));
        session.mark_authenticated();
        session.mark_initialized();
        session.update_focus(Some("control-7".to_owned()), Some("window-2".to_owned()));
        session.set_persistent_agent_active(true);
        session.set_nuone_service_mode(true);

        assert_eq!(session.username(), Some("erp-user"));
        assert!(session.authenticated());
        assert!(session.initialized());
        assert_eq!(session.focus_control_id(), Some("control-7"));
        assert_eq!(session.focus_parent_id(), Some("window-2"));
        assert!(session.persistent_agent_active());
        assert!(session.nuone_service_mode());
    }
}
