//! Static operational health response.

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub version: String,
    pub mode_suffix: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ActuatorHealthStatus {
    pub status: &'static str,
}

#[derive(Clone, Debug)]
pub struct HealthCheckService {
    status: HealthStatus,
}

impl HealthCheckService {
    pub fn new(version: impl Into<String>, mode_suffix: impl Into<String>) -> Self {
        Self {
            status: HealthStatus {
                version: version.into(),
                mode_suffix: mode_suffix.into(),
            },
        }
    }

    pub fn check(&self) -> HealthStatus {
        self.status.clone()
    }

    pub fn actuator_check(&self) -> ActuatorHealthStatus {
        ActuatorHealthStatus { status: "UP" }
    }
}
