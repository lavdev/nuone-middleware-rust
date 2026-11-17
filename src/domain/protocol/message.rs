#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtoMessage {
    pub header: String,
    pub payload: Vec<String>,
    pub raw: Vec<u8>,
}
