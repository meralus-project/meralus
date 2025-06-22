mod curves;
mod player;
mod value;

use std::{ops::Range, time::Duration};

use ahash::{HashMap, HashMapExt};
use meralus_shared::Lerp;

pub use self::{
    curves::{Curve, ICurve},
    player::AnimationPlayer,
    value::TweenValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RepeatMode {
    Once,
    Times(u16),
    Infinite,
}

impl RepeatMode {
    /// Returns `true` if the repeat mode is [`Infinite`].
    ///
    /// [`Infinite`]: RepeatMode::Infinite
    #[must_use]
    pub const fn is_infinite(&self) -> bool {
        matches!(self, Self::Infinite)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestartBehaviour {
    StartValue,
    EndValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FinishBehaviour {
    Reset,
    StaySame,
}

impl RestartBehaviour {
    /// Returns `true` if the restart behaviour is [`EndValue`].
    ///
    /// [`EndValue`]: RestartBehaviour::EndValue
    #[must_use]
    pub const fn is_end_value(self) -> bool {
        matches!(self, Self::EndValue)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Animation {
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    values: HashMap<String, (Curve, TweenValue)>,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_value<T: Into<String>, V: Into<TweenValue>>(
        mut self,
        name: T,
        value: V,
        curve: Curve,
    ) -> Self {
        self.values.insert(name.into(), (curve, value.into()));

        self
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Transition {
    elapsed: f32,
    duration: f32,
    delay: f32,
    curve: Curve,
    repeat: RepeatMode,
    restart_behaviour: RestartBehaviour,
    finish_behaviour: FinishBehaviour,

    origin: TweenValue,
    value: TweenValue,
    destination: TweenValue,
}

impl Transition {
    #[must_use]
    pub fn new<T: Into<TweenValue>>(
        start: T,
        end: T,
        duration: u64,
        curve: Curve,
        repeat: RepeatMode,
    ) -> Self {
        let [origin, destination] = [start.into(), end.into()];

        Self {
            elapsed: 0.0,
            delay: 0.0,
            duration: Duration::from_millis(duration).as_secs_f32(),
            curve,
            repeat,
            restart_behaviour: RestartBehaviour::StartValue,
            finish_behaviour: FinishBehaviour::StaySame,
            origin,
            value: origin,
            destination,
        }
    }

    #[must_use]
    pub fn new_with_delay<T: Into<TweenValue>>(
        start: T,
        end: T,
        duration: u64,
        delay: u64,
        curve: Curve,
        repeat: RepeatMode,
    ) -> Self {
        let [origin, destination] = [start.into(), end.into()];

        Self {
            elapsed: 0.0,
            delay: Duration::from_millis(delay).as_secs_f32(),
            duration: Duration::from_millis(duration).as_secs_f32(),
            curve,
            repeat,
            restart_behaviour: RestartBehaviour::StartValue,
            finish_behaviour: FinishBehaviour::StaySame,
            origin,
            value: origin,
            destination,
        }
    }

    #[must_use]
    pub const fn with_restart_behaviour(mut self, behaviour: RestartBehaviour) -> Self {
        self.restart_behaviour = behaviour;

        self
    }

    #[must_use]
    pub const fn with_finish_behaviour(mut self, behaviour: FinishBehaviour) -> Self {
        self.finish_behaviour = behaviour;

        self
    }

    pub fn set<T: Into<TweenValue>>(&mut self, value: T) {
        let value = value.into();

        self.origin = value;
        self.value = value;
    }

    pub fn set_value<T: Into<TweenValue>>(&mut self, value: T) {
        self.value = value.into();
    }

    pub fn to<T: Into<TweenValue>>(&mut self, value: T) {
        self.origin = self.value;
        self.destination = value.into();
    }

    pub fn get<T: From<TweenValue>>(&self) -> T {
        self.value.into()
    }

    pub const fn is_backwards(&self) -> bool {
        self.restart_behaviour.is_end_value()
    }

    pub const fn get_duration(&self) -> f32 {
        self.delay + self.duration
    }

    pub const fn get_elapsed(&self) -> f32 {
        match self.repeat {
            RepeatMode::Once | RepeatMode::Infinite => {
                if self.repeat.is_infinite()
                    && self.is_backwards()
                    && self.elapsed >= self.get_duration()
                {
                    self.get_duration()
                        - (self.elapsed.min(self.get_duration() * 2.0) - self.get_duration())
                } else {
                    self.elapsed.min(self.get_duration())
                }
            }
            RepeatMode::Times(_) => {
                self.elapsed.min(self.get_duration()) % (self.get_duration() + 1.0)
            }
        }
    }

    pub const fn reset(&mut self) {
        self.elapsed = 0.0;
        self.value = self.origin;
    }

    pub const fn set_delay(&mut self, delay: u64) {
        self.delay = Duration::from_millis(delay).as_secs_f32();
    }

    pub fn advance(&mut self, delta: f32) {
        self.elapsed += delta;

        if self.elapsed < self.delay {
            return;
        }

        let elapsed = self.elapsed - self.delay;

        let t = match (self.repeat, self.restart_behaviour) {
            (RepeatMode::Once, _) | (RepeatMode::Infinite, RestartBehaviour::StartValue) => {
                elapsed.min(self.duration) / self.duration
            }
            (RepeatMode::Times(_), _) => {
                (elapsed.min(self.duration) % (self.duration + 1.0)) / self.duration
            }
            (RepeatMode::Infinite, RestartBehaviour::EndValue) => {
                if elapsed >= self.duration {
                    (self.duration - (elapsed.min(self.duration * 2.0) - self.duration))
                        / self.duration
                } else {
                    elapsed.min(self.duration) / self.duration
                }
            }
        };

        self.value = self.origin.lerp(&self.destination, self.curve.transform(t));

        if self.repeat.is_infinite()
            && elapsed
                >= if self.restart_behaviour.is_end_value() {
                    self.duration * 2.0
                } else {
                    self.duration
                }
        {
            self.elapsed = 0.0;
        }
    }

    pub const fn is_finished(&self) -> bool {
        match self.repeat {
            RepeatMode::Once => self.elapsed >= self.get_duration(),
            RepeatMode::Times(n) => self.elapsed >= (self.get_duration() * n as f32),
            RepeatMode::Infinite => false,
        }
    }
}
