#[derive(Debug, Clone)]
pub struct Counter {
    pub skip: u64,
    pub count: Option<u64>,
    pub current: u64,
}

impl Counter {
    pub fn new(skip: u64, count: Option<u64>) -> Self {
        Self {
            skip,
            count,
            current: 0,
        }
    }

    pub fn next(&mut self) {
        self.current += 1;
    }

    pub fn included(&self) -> bool {
        if self.current <= self.skip {
            false
        } else if let Some(count) = self.count {
            self.current <= (self.skip + count)
        } else {
            true
        }
    }

    pub fn finished(&self) -> bool {
        if let Some(count) = self.count {
            self.current > self.skip + count
        } else {
            false
        }
    }
}
