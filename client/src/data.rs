use std::{collections::HashMap, io};

use bevy::{
    asset::{AssetLoader, LoadContext, ParseAssetPathError, io::Reader},
    prelude::*,
};
use super_tux_showdown_common::Character;
pub use super_tux_showdown_common::anim::CharacterAnimation;
use thiserror::Error;

pub fn plugin(app: &mut App) {
    app.init_asset::<CharacterDescription>()
        .init_asset_loader::<CharacterDescriptionLoader>();
}

#[derive(Asset, TypePath, Debug)]
pub struct CharacterDescription {
    pub name: String,
    pub model: Handle<Scene>,
    pub forward_rot: Quat,
    pub right_rot: Quat,
    pub left_rot: Quat,
    pub anims: HashMap<String, CharacterAnimation>,
}

#[derive(Default, TypePath)]
struct CharacterDescriptionLoader;

#[derive(Debug, Error)]
pub enum CharacterDescriptionLoaderError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    ParseAssetPathError(#[from] ParseAssetPathError),
}

impl AssetLoader for CharacterDescriptionLoader {
    type Asset = CharacterDescription;

    type Settings = ();

    type Error = CharacterDescriptionLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await?;
        let char: Character = serde_json::from_slice(&buf)?;
        Ok(CharacterDescription {
            name: char.name,
            model: load_context.load(
                GltfAssetLabel::Scene(0).from_asset(
                    load_context
                        .path()
                        .parent()
                        .unwrap()
                        .resolve(&char.model_file)?,
                ),
            ),
            forward_rot: char.forward_rot,
            right_rot: char.right_rot,
            left_rot: char.left_rot,
            anims: char.anims,
        })
    }
}
