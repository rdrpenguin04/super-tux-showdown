use std::collections::HashMap;

use bevy_math::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};

use crate::anim::CharacterAnimation;

pub mod anim;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, Reflect)]
pub struct TerrainBox {
    pub top: Vec2,
    pub bottom: Vec2,
    pub left: Vec2,
    pub right: Vec2,
}

impl TerrainBox {
    pub fn flip(self) -> Self {
        Self {
            top: self.top * vec2(-1.0, 1.0),
            bottom: self.bottom * vec2(-1.0, 1.0),
            left: self.left * vec2(-1.0, 1.0),
            right: self.right * vec2(-1.0, 1.0),
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct Character {
    pub name: String,
    pub model_file: String,
    pub forward_rot: Quat,
    pub right_rot: Quat,
    pub left_rot: Quat,
    pub anims: HashMap<String, CharacterAnimation>,
    #[serde_as(as = "Base64")]
    pub editor_data: Vec<u8>,
}
