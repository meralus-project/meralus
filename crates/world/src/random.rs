pub struct Random(i64);

impl Random {
    pub const fn new(seed: i64) -> Self {
        Self((seed ^ 0x5DEECE66Di64) & ((1i64 << 48) - 1))
    }

    pub const fn set_seed(&mut self, seed: i64) {
        self.0 = (seed ^ 0x5DEECE66Di64) & ((1i64 << 48) - 1);
    }

    const fn next(&mut self, bits: i64) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66Di64).wrapping_add(0xBi64)) & ((1i64 << 48) - 1);

        ((self.0 as u64) >> (48 - bits)) as i32
    }

    pub const fn next_i32(&mut self, range: i32) -> i32 {
        if (range & -range) == range {
            return ((range as i64).wrapping_mul(self.next(31) as i64) >> 31) as i32;
        }

        let mut bits: i32;
        let mut val: i32;

        loop {
            bits = self.next(31);
            val = bits % range;

            if !bits - val + (range - 1) < 0 {
                break;
            }
        }

        val
    }

    pub const fn next_i64(&mut self) -> i64 {
        ((self.next(32) as i64) << 32) + self.next(32) as i64
    }

    pub const fn next_f64(&mut self) -> f64 {
        (((self.next(26) as i64) << 27) + self.next(27) as i64) as f64 / (1i64 << 53) as f64
    }
}
