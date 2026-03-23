use std::collections::HashMap;

use bevy_math::{Quat, Vec2};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};

use crate::anim::CharacterAnimation;

pub mod anim;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct TerrainBox {
    pub top: Vec2,
    pub bottom: Vec2,
    pub left: Vec2,
    pub right: Vec2,
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
