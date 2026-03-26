use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::TerrainBox;

pub mod names;

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterAnimation {
    /// How should this animation be mirrored for when the character is facing
    /// left?
    pub mirror_rule: MirrorRule,
    /// What should happen when the end of the animation is reached?
    pub end_behavior: AnimEnd,
    /// The actual "frame data" of the animation
    pub frames: Vec<Frame>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirrorRule {
    /// Play the same animation. Useful for animations where the side of the
    /// body actions happen on matters, such as attacks (particularly sword
    /// strikes).
    NoChange,
    /// Use the mirrored copies of bones if they exist to play the animation.
    /// Useful for animations where you want the animation to look similar from
    /// the camera's perspective, such as pivots.
    MirrorBones,
    /// Use an entirely separate animation. Useful when no other option quite
    /// does what you need it to do.
    Separate { name: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnimEnd {
    /// Loop back to a specific frame. Particularly useful for run cycles and
    /// idle animations.
    Loop { frame: usize },
    /// End the animation. Character logic should enter a new state and start a
    /// new animation.
    End,
}

#[derive(Serialize, Deserialize, Debug, Reflect, Default, Clone)]
pub struct Frame {
    /// Flag to ensure this frame doesn't get its display skipped unless
    /// absolutely necessary.
    ///
    /// Specifically, there are only two times an important frame should ever be
    /// skipped:
    /// * A new animation has started. A character getting hit and entering
    ///   hitstun is a bigger priority than showing a missed jab.
    /// * The rendering is behind enough that the animation should be on another
    ///   important frame. The later important frame should be prioritized to
    ///   keep the animation lined up closely with action physics.
    pub important: bool,
    /// Physical coordinates of left/right/top/bottom points of the model. Used
    /// to set the size of the terrain collider.
    pub bounding_box: TerrainBox,
    // TODO: actual animation data, including hitboxes and bone transforms
}
