mod curves;

use std::time::Duration;

use meralus_shared::Lerp;

pub use self::curves::{Curve, ICurve};

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

impl FinishBehaviour {
    /// Returns `true` if the finish behaviour is [`Reset`].
    ///
    /// [`Reset`]: FinishBehaviour::Reset
    #[must_use]
    pub const fn is_reset(&self) -> bool {
        matches!(self, Self::Reset)
    }
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

pub trait Animation {
    type Item;

    fn set_elapsed(&mut self, elapsed: Duration);
    fn advance(&mut self, delta: Duration) -> AnimationResult;
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Tween<T: Lerp> {
    // Tween state
    /// Current value.
    value: T,
    /// Value from which we are transitioning.
    origin: T,
    /// Value to which we are transitioning.
    target: T,
    /// Time elapsed since start of transition (in milliseconds).
    elapsed: u64,

    // Tween config
    /// Time that transition takes to finish (in milliseconds). Does not include
    /// delay.
    duration: u64,
    /// Time that transition takes to really start (in milliseconds).
    delay: u64,
    /// Curve used for transitioning from [`origin`] to [`target`].
    curve: Curve,
    /// Describes how this transition will be animated. Can be
    /// [`RepeatMode::Once`], [`RepeatMode::Times`] or [`RepeatMode::Infinite`].
    repeat: RepeatMode,
    restart_behaviour: RestartBehaviour,
    finish_behaviour: FinishBehaviour,
}

impl<T: Lerp + Clone> Tween<T> {
    #[must_use]
    pub fn new(origin: T, target: T, duration: u64) -> Self {
        Self {
            elapsed: 0,
            delay: 0,
            duration,
            curve: Curve::LINEAR,
            repeat: RepeatMode::Once,
            restart_behaviour: RestartBehaviour::StartValue,
            finish_behaviour: FinishBehaviour::StaySame,
            origin: origin.clone(),
            value: origin,
            target,
        }
    }

    pub fn with_repeat_mode(mut self, mode: RepeatMode) -> Self {
        self.repeat = mode;

        self
    }

    pub fn with_curve(mut self, curve: Curve) -> Self {
        self.curve = curve;

        self
    }

    #[must_use]
    pub fn with_delay(mut self, delay: u64) -> Self {
        self.delay = delay;

        self
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

    pub fn set(&mut self, value: T) {
        self.elapsed = 0;
        self.origin = self.value.clone();
        self.target = value;
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub const fn is_backwards(&self) -> bool {
        self.restart_behaviour.is_end_value()
    }

    pub const fn get_duration(&self) -> u64 {
        self.delay + self.duration
    }

    pub fn get_elapsed(&self) -> u64 {
        match self.repeat {
            RepeatMode::Once => self.elapsed.min(self.get_duration()),
            RepeatMode::Times(_) => self.elapsed.min(self.get_duration()) % (self.get_duration() + 1),
            RepeatMode::Infinite => {
                if self.is_backwards() && self.elapsed >= self.get_duration() {
                    self.get_duration() - (self.elapsed.min(self.get_duration() * 2) - self.get_duration())
                } else {
                    self.elapsed.min(self.get_duration())
                }
            }
        }
    }

    pub const fn is_finished(&self) -> bool {
        match self.repeat {
            RepeatMode::Once => self.elapsed >= self.get_duration(),
            RepeatMode::Times(n) => self.elapsed >= (self.get_duration() * n as u64),
            RepeatMode::Infinite => false,
        }
    }

    fn advance_value(&mut self, delta: Duration) {
        let delta = delta.as_millis() as u64;

        self.elapsed = self.elapsed.saturating_add(delta);

        if self.elapsed >= self.delay {
            let elapsed = self.elapsed.saturating_sub(self.delay);

            self.value = self.origin.lerp(&self.target, self.curve.transform(elapsed as f32 / self.duration as f32));
        }
    }
}

impl<T: Lerp + Copy> Tween<T> {
    pub fn get_copy(&self) -> T {
        self.value
    }
}

impl<T: Lerp + Clone> Animation for Tween<T> {
    type Item = T;

    fn set_elapsed(&mut self, _: Duration) {}

    fn advance(&mut self, delta: Duration) -> AnimationResult {
        if self.is_finished() {
            if self.finish_behaviour.is_reset() {
                self.value = self.origin.clone();
            }

            AnimationResult::Finished
        } else {
            self.advance_value(delta);

            AnimationResult::InProgress
        }
    }
}

macro_rules! impl_tuple_anim {
    ($($generic:ident => $pos:tt),*) => {
        impl<$($generic: Lerp + Clone),*> Animation for ($(Tween<$generic>),*) {
            type Item = ($($generic),*);

            fn set_elapsed(&mut self, _: Duration) {}

            fn advance(&mut self, delta: Duration) -> AnimationResult {
                $(self.$pos.advance(delta);)*

                AnimationResult::InProgress
            }
        }
    };
}

pub struct Frame<T: Lerp> {
    pub range: [u64; 2],
    pub duration: u64,
    pub value: T,
    pub curve: Curve,
}

pub struct KeyframeAnimation<T: Lerp> {
    elapsed: u64,
    duration: u64,
    initial: T,
    value: T,
    frames: Vec<Frame<T>>,
    current_frame: Option<usize>,
}

impl<T: Lerp + Copy> KeyframeAnimation<T> {
    pub fn new(duration: u64, value: T) -> Self {
        Self {
            elapsed: 0,
            duration,
            initial: value,
            value,
            frames: Vec::new(),
            current_frame: None,
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = 0;
        self.value = self.initial;
    }
}

impl<T: Lerp> KeyframeAnimation<T> {
    pub fn previous_frame(&mut self) -> Option<usize> {
        self.current_frame().and_then(|frame| if frame == 0 { None } else { Some(frame - 1) })
    }

    pub fn set_initial(&mut self, value: T) {
        self.initial = value;
    }

    // [0  , 100] elapsed: 50, 100 >= 50 && 0 < 50
    // [100, 300] elapsed: 50
    // [300, 400] elapsed: 50

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn elapsed(&self) -> u64 {
        self.elapsed
    }

    pub fn duration(&self) -> u64 {
        self.duration
    }

    pub fn frame_mut(&mut self, start: u64, end: u64) -> Option<&mut Frame<T>> {
        self.frames.iter_mut().find(|frame| frame.range == [start, end])
    }

    // 96
    //
    // [0  , 200] 0 < 96, 200 >= 96
    // [600, 800] 800 >= 96

    pub fn current_frame(&mut self) -> Option<usize> {
        let frame = self
            .frames
            .iter()
            .position(|frame| frame.range[0] < self.elapsed && frame.range[1] >= self.elapsed);

        if let Some(frame) = frame {
            self.current_frame.replace(frame);

            Some(frame)
        } else {
            self.current_frame
        }
    }

    pub fn frame_at(mut self, start: u64, end: u64, value: T, curve: Curve) -> Self {
        self.frames.push(Frame {
            range: [start, end],
            duration: end - start,
            value,
            curve,
        });

        self
    }

    pub const fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    fn advance_value(&mut self) {
        if let Some(idx) = self.current_frame() {
            let duration = self.frames[idx].duration;

            if self.elapsed >= self.frames[idx].range[0] {
                let t = (self.elapsed - self.frames[idx].range[0]).min(duration) as f32 / duration as f32;
                let value = match self.previous_frame() {
                    Some(idx) => &self.frames[idx].value,
                    None => &self.initial,
                };

                self.value = value.lerp(&self.frames[idx].value, self.frames[idx].curve.transform(t));
            }
        }
    }
}

impl<T: Lerp + Copy> KeyframeAnimation<T> {
    pub fn get_copy(&self) -> T {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnimationResult {
    Finished,
    InProgress,
}

impl<T: Lerp> Animation for KeyframeAnimation<T> {
    type Item = T;

    fn set_elapsed(&mut self, _: Duration) {}

    fn advance(&mut self, delta: Duration) -> AnimationResult {
        if !self.is_finished() {
            self.elapsed += delta.as_millis() as u64;
            self.advance_value();

            AnimationResult::InProgress
        } else if self.elapsed != self.duration {
            self.elapsed = self.duration;
            self.advance_value();

            AnimationResult::InProgress
        } else {
            AnimationResult::Finished
        }
    }
}

impl_tuple_anim![A => 0, B => 1];
impl_tuple_anim![A => 0, B => 1, C => 2];
impl_tuple_anim![A => 0, B => 1, C => 2, D => 3];
impl_tuple_anim![A => 0, B => 1, C => 2, D => 3, E => 4];

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{Animation, Curve, KeyframeAnimation, Tween};

    #[test]
    fn test_keyframes() {
        let mut text_animation = KeyframeAnimation::new(400, 1.0)
            .frame_at(0, 100, 0.0, Curve::LINEAR)
            .frame_at(300, 400, 1.0, Curve::LINEAR);

        let mut animation = KeyframeAnimation::new(400, 10.0).frame_at(100, 300, 200.0, Curve::LINEAR);

        while !text_animation.is_finished() || !animation.is_finished() {
            println!("{}ms: {}, {}", text_animation.elapsed, text_animation.value, animation.value);

            text_animation.advance(Duration::from_millis(10));
            animation.advance(Duration::from_millis(10));
        }

        println!("{}ms: {}, {}", text_animation.elapsed, text_animation.value, animation.value);
    }

    #[test]
    fn test_tween() {
        let mut tween = Tween::new(0.0, 1.0, 400);

        while !tween.is_finished() {
            println!("{}ms: {}", tween.elapsed, tween.value);

            tween.advance(Duration::from_millis(10));

            if tween.elapsed == 200 {
                tween.set(0.0);
            } else if tween.elapsed == 300 {
                tween.set(1.0);
            }
        }

        println!("{}ms: {}", tween.elapsed, tween.value);

        let mut tween = Tween::new(0.0, 1.0, 400);

        while !tween.is_finished() {
            println!("{}ms: {}", tween.elapsed, tween.value);

            tween.advance(Duration::from_millis(10));

            if tween.elapsed == 200 {
                tween.set(0.0);
            } else if tween.elapsed == 300 {
                tween.set(1.0);
            }
        }

        println!("{}ms: {}", tween.elapsed, tween.value);
    }
}
