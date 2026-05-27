#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDeviceMetadata {
    pub device_code: u8,
    pub shared_with: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerMetadata {
    pub device_code: u8,
    pub instruction_name: &'static str,
}
