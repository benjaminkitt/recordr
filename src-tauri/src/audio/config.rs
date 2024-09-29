use cpal::traits::DeviceTrait;
use cpal::{Device, SupportedStreamConfig};
use std::fmt;

pub struct DeviceWrapper(pub Device);

impl fmt::Debug for DeviceWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Device({})", self.0.name().unwrap_or_default())
    }
}

#[derive(Debug)]
pub struct AudioConfig {
    pub device: DeviceWrapper,
    pub config: SupportedStreamConfig,
    pub sample_rate: usize,
}

pub enum AudioEvent {
    Voice,
    Silence,
}

// Enum for recording state
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordingState {
    Idle,
    Recording,
    Paused,
}