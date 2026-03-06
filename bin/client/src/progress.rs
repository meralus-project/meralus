use std::{borrow::Cow, mem::replace, sync::mpsc};

use glium::{
    Rect, Texture2d,
    texture::{ClientFormat, RawImage2d},
};
use meralus_animation::AnimationPlayer;
use meralus_storage::ResourceStorage;

pub enum ProgressChange {
    SetInitialInfo(ProgressInfo),
    NewStage(String, usize),
    TaskCompleted,
    SetVisible(bool),
}

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub total_stages: usize,
    pub current_stage: usize,
    pub current_stage_name: Option<String>,
    pub total: usize,
    pub completed: usize,
}

impl ProgressInfo {
    pub const fn new(total_stages: usize, current_stage: usize, total: usize, completed: usize) -> Self {
        Self {
            total_stages,
            current_stage,
            current_stage_name: None,
            total,
            completed,
        }
    }
}

pub struct Progress {
    receiver: mpsc::Receiver<ProgressChange>,
    pub info: Option<ProgressInfo>,
    pub visible: bool,
}

impl Progress {
    pub const fn new(receiver: mpsc::Receiver<ProgressChange>) -> Self {
        Self {
            receiver,
            info: None,
            visible: false,
        }
    }

    pub fn update(&mut self, animation: &mut AnimationPlayer, texture: &Texture2d, lightmap: &Texture2d, resource_manager: &ResourceStorage) {
        if let Ok(info) = self.receiver.try_recv() {
            match info {
                ProgressChange::SetInitialInfo(info) => {
                    self.info.replace(info);
                }
                ProgressChange::NewStage(name, tasks) => {
                    if let Some(info) = &mut self.info {
                        info.current_stage += 1;
                        info.current_stage_name.replace(name);
                        info.completed = 0;

                        let previous_tasks = replace(&mut info.total, tasks);

                        animation.play_transition_to("stage-progress", info.current_stage as f32 / info.total_stages as f32);
                        animation.play_transition_to("stage-substage-progress", 0.0);

                        {
                            let anim = animation.get_mut_unchecked("stage-previous-progress");

                            anim.set_value((previous_tasks - 1) as f32 / previous_tasks as f32);
                            anim.to(1.0);
                        };

                        animation.play("stage-previous-progress");
                        animation.play("stage-substage-translation");
                    }
                }
                ProgressChange::TaskCompleted => {
                    if let Some(info) = &mut self.info {
                        info.completed += 1;

                        animation.play_transition_to("stage-substage-progress", info.completed as f32 / info.total as f32);
                    }
                }
                ProgressChange::SetVisible(visible) => {
                    self.visible = visible;

                    animation.play_transition_to("progress-opacity", f32::from(visible));

                    if visible {
                        animation.play("text-scaling");
                        animation.play("shape-morph");
                    }

                    if !visible {
                        for (mipmap, image) in resource_manager.get_mipmaps().iter().enumerate() {
                            if let Some(texture) = texture.mipmap(mipmap as u32) {
                                texture.write(
                                    Rect {
                                        left: 0,
                                        bottom: 0,
                                        width: image.width(),
                                        height: image.height(),
                                    },
                                    RawImage2d {
                                        data: Cow::Borrowed(image.as_raw()),
                                        width: image.width(),
                                        height: image.height(),
                                        format: ClientFormat::U8U8U8U8,
                                    },
                                );
                            }
                        }

                        for (mipmap, image) in resource_manager.get_lightmap_mipmaps().iter().enumerate() {
                            if let Some(lightmap) = lightmap.mipmap(mipmap as u32) {
                                lightmap.write(
                                    Rect {
                                        left: 0,
                                        bottom: 0,
                                        width: image.width(),
                                        height: image.height(),
                                    },
                                    RawImage2d {
                                        data: Cow::Borrowed(image.as_raw()),
                                        width: image.width(),
                                        height: image.height(),
                                        format: ClientFormat::U8U8U8U8,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct ProgressSender(pub mpsc::Sender<ProgressChange>);

impl ProgressSender {
    pub fn set_initial_info(&self, info: ProgressInfo) -> Result<(), mpsc::SendError<ProgressChange>> {
        self.0.send(ProgressChange::SetInitialInfo(info))
    }

    pub fn new_stage<T: Into<String>>(&self, name: T, tasks: usize) -> Result<(), mpsc::SendError<ProgressChange>> {
        self.0.send(ProgressChange::NewStage(name.into(), tasks))
    }

    pub fn complete_task(&self) -> Result<(), mpsc::SendError<ProgressChange>> {
        self.0.send(ProgressChange::TaskCompleted)
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), mpsc::SendError<ProgressChange>> {
        self.0.send(ProgressChange::SetVisible(visible))
    }
}
