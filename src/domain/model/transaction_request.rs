use super::super::protocol::{PROTO_SEPARATOR, encode_proto_text};

#[derive(Clone, PartialEq, Eq)]
pub struct TransactionRequest {
    pub transaction: String,
    pub username: String,
    pub password: String,
    pub customer_id: String,
}

impl TransactionRequest {
    pub fn to_proto(&self) -> String {
        [
            "\u{00a4}TS",
            self.customer_id.as_str(),
            self.username.as_str(),
            self.password.as_str(),
            "0",
            "90",
            self.transaction.as_str(),
        ]
        .join(PROTO_SEPARATOR)
    }

    pub fn to_proto_bytes(&self) -> Vec<u8> {
        encode_proto_text(&self.to_proto())
    }
}

#[cfg(test)]
mod tests {
    use super::TransactionRequest;

    #[test]
    fn serializes_to_the_legacy_transaction_string_and_bytes() {
        let request = TransactionRequest {
            transaction: "PAYMENT_PIX".to_owned(),
            username: "erp-user".to_owned(),
            password: "secret".to_owned(),
            customer_id: "tenant-1".to_owned(),
        };

        assert_eq!(
            request.to_proto(),
            "¤TS÷tenant-1÷erp-user÷secret÷0÷90÷PAYMENT_PIX"
        );
        assert_eq!(
            request.to_proto_bytes(),
            [
                0xa4, 0x54, 0x53, 0xf7, 0x74, 0x65, 0x6e, 0x61, 0x6e, 0x74, 0x2d, 0x31, 0xf7, 0x65,
                0x72, 0x70, 0x2d, 0x75, 0x73, 0x65, 0x72, 0xf7, 0x73, 0x65, 0x63, 0x72, 0x65, 0x74,
                0xf7, 0x30, 0xf7, 0x39, 0x30, 0xf7, 0x50, 0x41, 0x59, 0x4d, 0x45, 0x4e, 0x54, 0x5f,
                0x50, 0x49, 0x58,
            ]
        );
    }
}
