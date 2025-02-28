use std::{cmp, ops::Range};

use esp_idf_hal::ledc::LedcDriver;

pub fn map_range(lhs: Range<i64>, rhs: Range<i64>, val: i64) -> i64 {
    if val == 0 {
        return 0;
    }

    rhs.start + ((val - lhs.start) * (rhs.end - rhs.start) / (lhs.end - lhs.start))
}

pub struct Motor {
    driver: LedcDriver<'static>,
    duty: u32,
}

impl Motor {
    pub fn new(driver: LedcDriver<'static>) -> Self {
        Self { driver, duty: 0 }
    }

    pub fn set(&mut self, power: u32) -> u32 {
        self.duty = cmp::min(power, 20);
        let mapped = map_range(0..20, 50..100, self.duty as i64) as u32;
        self.driver
            .set_duty(self.driver.get_max_duty() * mapped / 100)
            .unwrap();
        self.duty
    }

    pub fn get(&self) -> u32 {
        self.duty
    }

    pub fn inc(&mut self) -> u32 {
        if self.duty < 20 {
            self.set(self.duty + 1)
        } else {
            self.duty
        }
    }

    pub fn dec(&mut self) -> u32 {
        if self.duty > 0 {
            self.set(self.duty - 1)
        } else {
            self.duty
        }
    }
}
