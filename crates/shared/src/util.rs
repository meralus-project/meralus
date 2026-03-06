use core::ops::{AddAssign, SubAssign};

pub trait AsValue<T> {
    fn as_value(&self) -> T;
}

pub trait FromValue<T> {
    fn from_value(value: &T) -> Self;
}

impl<A, T> FromValue<T> for A
where
    T: AsValue<A>,
{
    fn from_value(value: &T) -> Self {
        value.as_value()
    }
}

pub trait InspectMut<T> {
    fn inspect_mut<F: FnOnce(&mut T)>(&mut self, func: F);
}

impl<T> InspectMut<T> for Option<T> {
    fn inspect_mut<F: FnOnce(&mut T)>(&mut self, func: F) {
        if let Some(data) = self {
            func(data);
        }
    }
}

pub trait Num {
    fn one() -> Self;
}

impl Num for usize {
    fn one() -> Self {
        1
    }
}

impl Num for u8 {
    fn one() -> Self {
        1
    }
}

pub struct Ranged<T> {
    pub min: T,
    pub max: T,
    pub value: T,
}

impl<T: Num + PartialOrd + SubAssign + AddAssign + Copy> Ranged<T> {
    pub const fn new(default_value: T, min: T, max: T) -> Self {
        Self {
            min,
            max,
            value: default_value,
        }
    }

    pub fn increase(&mut self) {
        if self.value == self.max {
            self.value = self.min;
        } else {
            self.value += T::one();
        }
    }

    pub fn decrease(&mut self) {
        if self.value == self.min {
            self.value = self.max;
        } else {
            self.value -= T::one();
        }
    }
}
