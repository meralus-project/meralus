use ahash::HashSet;
use indexmap::IndexMap;

use crate::{Transition, FinishBehaviour, TweenValue};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnimationPlayer {
    animations: IndexMap<String, Transition>,
    running: HashSet<String>,
    enabled: bool,
}

impl AnimationPlayer {
    pub fn reset(&mut self) {
        for animation in self.animations.values_mut() {
            animation.reset();
        }
    }

    pub const fn is_enabled(&mut self) -> bool {
        self.enabled
    }

    pub const fn enable(&mut self) {
        self.enabled = true;
    }

    pub const fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn advance(&mut self, delta: f32) {
        if self.enabled {
            for (name, animation) in &mut self.animations {
                if self.running.contains(name) {
                    animation.advance(delta);

                    if animation.is_finished() {
                        if animation.finish_behaviour == FinishBehaviour::Reset {
                            animation.value = animation.origin;
                        }

                        self.running.remove(name);
                    }
                }
            }
        }
    }

    pub fn add<T: Into<String>>(&mut self, name: T, animation: Transition) {
        self.animations.insert(name.into(), animation);
    }

    pub fn get_at(&self, index: usize) -> Option<(&str, &Transition)> {
        self.animations
            .get_index(index)
            .map(|(name, animation)| (name.as_str(), animation))
    }

    pub fn get<T: AsRef<str>>(&mut self, name: T) -> Option<&Transition> {
        self.animations.get(name.as_ref())
    }

    pub fn get_mut<T: AsRef<str>>(&mut self, name: T) -> Option<&mut Transition> {
        self.animations.get_mut(name.as_ref())
    }

    pub fn play<T: Into<String>>(&mut self, name: T) {
        let name = name.into();

        if let Some(animation) = self.animations.get_mut(&name) {
            animation.reset();

            self.running.insert(name);
        }
    }

    pub fn play_transition_to<T: Into<String>, V: Into<TweenValue>>(&mut self, name: T, value: V) {
        let name = name.into();

        if let Some(animation) = self.animations.get_mut(&name) {
            animation.to(value);
            animation.reset();

            self.running.insert(name);
        }
    }

    pub fn get_elapsed<T: AsRef<str>>(&self, name: T) -> Option<f32> {
        self.animations
            .get(name.as_ref())
            .map(Transition::get_elapsed)
    }

    pub fn get_duration<T: AsRef<str>>(&self, name: T) -> Option<f32> {
        self.animations
            .get(name.as_ref())
            .map(|animation| animation.duration)
    }

    pub fn get_value<T: AsRef<str>, V: From<TweenValue>>(&self, name: T) -> Option<V> {
        self.animations.get(name.as_ref()).map(Transition::get)
    }

    pub fn animations(&self) -> impl Iterator<Item = (&str, &Transition)> {
        self.animations
            .iter()
            .map(|(name, animation)| (name.as_str(), animation))
    }

    pub fn len(&self) -> usize {
        self.animations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    pub fn is_finished<T: AsRef<str>>(&self, name: T) -> bool {
        self.enabled
            && self
                .animations
                .get(name.as_ref())
                .is_some_and(Transition::is_finished)
    }
}
