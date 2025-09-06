use bevy::prelude::*;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct SimPause(pub bool);

