//! High-level types and command helpers for the ESP-Keiretsu leadscrew system.

use core::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Feed,
    Thread,
    Jog,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feed => write!(f, "FEED"),
            Self::Thread => write!(f, "THREAD"),
            Self::Jog => write!(f, "JOG"),
        }
    }
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .to_ascii_uppercase()
            .as_str()
        {
            "FEED" => Ok(Self::Feed),
            "THREAD" => Ok(Self::Thread),
            "JOG" => Ok(Self::Jog),
            _ => Err(format!("unknown mode '{s}', expected FEED, THREAD, or JOG")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward => write!(f, "FWD"),
            Self::Reverse => write!(f, "REV"),
        }
    }
}

impl FromStr for Direction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .to_ascii_uppercase()
            .as_str()
        {
            "FWD" | "FORWARD" => Ok(Self::Forward),
            "REV" | "REVERSE" => Ok(Self::Reverse),
            _ => Err(format!("unknown direction '{s}', expected FWD or REV")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Run,
    Off,
    Fault,
}

impl fmt::Display for MachineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run => write!(f, "RUN"),
            Self::Off => write!(f, "OFF"),
            Self::Fault => write!(f, "FAULT"),
        }
    }
}

impl FromStr for MachineState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .to_ascii_uppercase()
            .as_str()
        {
            "RUN" => Ok(Self::Run),
            "OFF" => Ok(Self::Off),
            "FAULT" => Ok(Self::Fault),
            _ => Err(format!("unknown state '{s}', expected RUN, OFF, or FAULT")),
        }
    }
}

/// Structured status parsed from `RPM=<n> POS=<n> TARGET=<n> STATE=<RUN|OFF|FAULT>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub rpm: i64,
    pub position: i64,
    pub target_steps: i64,
    pub state: MachineState,
}

impl Status {
    pub fn parse(payload: &str) -> Result<Self, String> {
        let mut rpm = None;
        let mut position = None;
        let mut target_steps = None;
        let mut state = None;

        for token in payload.split_ascii_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                match k {
                    "RPM" => {
                        rpm = v
                            .parse::<i64>()
                            .ok()
                    }
                    "POS" => {
                        position = v
                            .parse::<i64>()
                            .ok()
                    }
                    "TARGET" => {
                        target_steps = v
                            .parse::<i64>()
                            .ok()
                    }
                    "STATE" => {
                        state = v
                            .parse::<MachineState>()
                            .ok()
                    }
                    _ => {}
                }
            }
        }

        Ok(Self {
            rpm: rpm.ok_or_else(|| "missing RPM in status payload".to_string())?,
            position: position.ok_or_else(|| "missing POS in status payload".to_string())?,
            target_steps: target_steps
                .ok_or_else(|| "missing TARGET in status payload".to_string())?,
            state: state.ok_or_else(|| "missing STATE in status payload".to_string())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_display_and_from_str() {
        assert_eq!(Mode::Feed.to_string(), "FEED");
        assert_eq!(Mode::Thread.to_string(), "THREAD");
        assert_eq!(Mode::Jog.to_string(), "JOG");

        assert_eq!(
            "feed"
                .parse::<Mode>()
                .unwrap(),
            Mode::Feed
        );
        assert_eq!(
            "FEED"
                .parse::<Mode>()
                .unwrap(),
            Mode::Feed
        );
        assert_eq!(
            "Thread"
                .parse::<Mode>()
                .unwrap(),
            Mode::Thread
        );
        assert_eq!(
            "THREAD"
                .parse::<Mode>()
                .unwrap(),
            Mode::Thread
        );
        assert_eq!(
            "jog"
                .parse::<Mode>()
                .unwrap(),
            Mode::Jog
        );
        assert_eq!(
            "JOG"
                .parse::<Mode>()
                .unwrap(),
            Mode::Jog
        );

        assert!(
            "invalid"
                .parse::<Mode>()
                .is_err()
        );
        assert!(
            "".parse::<Mode>()
                .is_err()
        );
    }

    #[test]
    fn direction_display_and_from_str() {
        assert_eq!(Direction::Forward.to_string(), "FWD");
        assert_eq!(Direction::Reverse.to_string(), "REV");

        assert_eq!(
            "fwd"
                .parse::<Direction>()
                .unwrap(),
            Direction::Forward
        );
        assert_eq!(
            "FWD"
                .parse::<Direction>()
                .unwrap(),
            Direction::Forward
        );
        assert_eq!(
            "forward"
                .parse::<Direction>()
                .unwrap(),
            Direction::Forward
        );
        assert_eq!(
            "FORWARD"
                .parse::<Direction>()
                .unwrap(),
            Direction::Forward
        );

        assert_eq!(
            "rev"
                .parse::<Direction>()
                .unwrap(),
            Direction::Reverse
        );
        assert_eq!(
            "REV"
                .parse::<Direction>()
                .unwrap(),
            Direction::Reverse
        );
        assert_eq!(
            "reverse"
                .parse::<Direction>()
                .unwrap(),
            Direction::Reverse
        );
        assert_eq!(
            "REVERSE"
                .parse::<Direction>()
                .unwrap(),
            Direction::Reverse
        );

        assert!(
            "left"
                .parse::<Direction>()
                .is_err()
        );
        assert!(
            "".parse::<Direction>()
                .is_err()
        );
    }

    #[test]
    fn machine_state_display_and_from_str() {
        assert_eq!(MachineState::Run.to_string(), "RUN");
        assert_eq!(MachineState::Off.to_string(), "OFF");
        assert_eq!(MachineState::Fault.to_string(), "FAULT");

        assert_eq!(
            "run"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Run
        );
        assert_eq!(
            "RUN"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Run
        );
        assert_eq!(
            "off"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Off
        );
        assert_eq!(
            "OFF"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Off
        );
        assert_eq!(
            "fault"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Fault
        );
        assert_eq!(
            "FAULT"
                .parse::<MachineState>()
                .unwrap(),
            MachineState::Fault
        );

        assert!(
            "idle"
                .parse::<MachineState>()
                .is_err()
        );
        assert!(
            "".parse::<MachineState>()
                .is_err()
        );
    }

    #[test]
    fn parse_status_payload_valid() {
        let status = Status::parse("RPM=1200 POS=-450 TARGET=-450 STATE=RUN").unwrap();
        assert_eq!(status.rpm, 1200);
        assert_eq!(status.position, -450);
        assert_eq!(status.target_steps, -450);
        assert_eq!(status.state, MachineState::Run);

        let zero_status = Status::parse("RPM=0 POS=0 TARGET=0 STATE=OFF").unwrap();
        assert_eq!(zero_status.rpm, 0);
        assert_eq!(zero_status.position, 0);
        assert_eq!(zero_status.target_steps, 0);
        assert_eq!(zero_status.state, MachineState::Off);

        let fault_status = Status::parse("RPM=-300 POS=100000 TARGET=-50000 STATE=FAULT").unwrap();
        assert_eq!(fault_status.rpm, -300);
        assert_eq!(fault_status.position, 100000);
        assert_eq!(fault_status.target_steps, -50000);
        assert_eq!(fault_status.state, MachineState::Fault);
    }

    #[test]
    fn parse_status_payload_shuffled_and_extra_tokens() {
        let status = Status::parse("STATE=RUN FOO=BAR POS=50 RPM=250 EXTRA=99 TARGET=75").unwrap();
        assert_eq!(status.rpm, 250);
        assert_eq!(status.position, 50);
        assert_eq!(status.target_steps, 75);
        assert_eq!(status.state, MachineState::Run);
    }

    #[test]
    fn parse_status_payload_missing_or_invalid_fields() {
        // Missing fields
        assert!(Status::parse("POS=100 TARGET=100 STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 TARGET=100 STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 POS=100 STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 POS=100 TARGET=100").is_err());
        assert!(Status::parse("").is_err());

        // Invalid integers or state
        assert!(Status::parse("RPM=abc POS=100 TARGET=100 STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 POS=xyz TARGET=100 STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 POS=100 TARGET=invalid STATE=RUN").is_err());
        assert!(Status::parse("RPM=100 POS=100 TARGET=100 STATE=INVALID").is_err());
    }
}
