use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Code
{
    pub camera_id: String,
    pub text: String,
    pub captured_at: SystemTime,
    pub restored: bool
}
