//! Sans-io RESP-over-x client protocol: pure command encoding and reply parsing, with no I/O
//! of its own -- identical regardless of what transport (BLE, TCP, ...) carries the bytes, and
//! unit-testable without a socket or radio.

use resp::RespFrame;

use crate::error::LinkError;
use crate::types::{Direction, Mode, Status};

/// Encodes a command line as bytes ready to hand to a transport, ensuring CRLF termination --
/// the resp-over-x wire format requires it regardless of what carries the bytes.
pub fn encode(command: &str) -> Vec<u8> {
    let mut data = command
        .as_bytes()
        .to_vec();
    if !data.ends_with(b"\r\n") {
        data.extend_from_slice(b"\r\n");
    }
    data
}

pub fn mode_command(mode: Mode) -> String {
    format!("MODE {mode}")
}

pub fn ratio_command(ratio_um: i64) -> String {
    format!("RATIO {ratio_um}")
}

pub fn direction_command(direction: Direction) -> String {
    format!("DIR {direction}")
}

/// Interprets a reply frame expected to be a bare acknowledgement.
pub fn parse_ok(frame: RespFrame) -> Result<(), LinkError> {
    match frame {
        RespFrame::Ok => Ok(()),
        RespFrame::Error(e) => Err(LinkError::Protocol(e)),
        _ => Err(LinkError::Protocol("unexpected response format".into())),
    }
}

/// Interprets a reply frame expected to carry a `STATUS` payload.
pub fn parse_status(frame: RespFrame) -> Result<Status, LinkError> {
    match frame {
        RespFrame::Bulk(payload) => Status::parse(&payload).map_err(LinkError::Protocol),
        RespFrame::Error(e) => Err(LinkError::Protocol(e)),
        _ => Err(LinkError::Protocol("unexpected response format".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_appends_crlf_only_when_missing() {
        assert_eq!(encode("STATUS"), b"STATUS\r\n");
        assert_eq!(encode("MODE FEED\r\n"), b"MODE FEED\r\n");
        assert_eq!(encode("RATIO 1500"), b"RATIO 1500\r\n");
    }

    #[test]
    fn parse_ok_accepts_ok_and_rejects_error_or_other() {
        assert!(parse_ok(RespFrame::Ok).is_ok());
        assert!(matches!(
            parse_ok(RespFrame::Error("busy".into())),
            Err(LinkError::Protocol(_))
        ));
        assert!(matches!(
            parse_ok(RespFrame::Bulk("x".into())),
            Err(LinkError::Protocol(_))
        ));
    }

    #[test]
    fn parse_status_reads_bulk_payload() {
        let status = parse_status(RespFrame::Bulk(
            "RPM=1200 POS=-450 TARGET=-450 STATE=RUN".into(),
        ))
        .unwrap();
        assert_eq!(status.rpm, 1200);

        assert!(matches!(
            parse_status(RespFrame::Error("nope".into())),
            Err(LinkError::Protocol(_))
        ));
        assert!(matches!(
            parse_status(RespFrame::Ok),
            Err(LinkError::Protocol(_))
        ));
    }
}
