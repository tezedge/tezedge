#[derive(Default, Debug, Clone)]
pub struct BlocksMonitor {
    pub current_level: Option<i32>,
}

impl BlocksMonitor {
    pub fn new() -> Self {
        Default::default()
    }
}