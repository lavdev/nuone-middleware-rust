use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub id: Option<i64>,
    pub uuid: String,
    pub filename: String,
    pub session_id: String,
    pub created_at: SystemTime,
}
