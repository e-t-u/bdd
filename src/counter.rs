#[derive(Debug, Clone)]
pub struct Counter {
    pub skip: usize,
    pub count: Option<usize>,
    pub current: usize,
}

impl Counter {
    pub fn new(skip: usize, count: Option<usize>) -> Self {
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
        } else if self.count.is_none() {
            true
        } else {
            self.current <= (self.skip + self.count.unwrap())
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
