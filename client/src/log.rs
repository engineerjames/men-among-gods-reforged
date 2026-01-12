// Custom logger module -- wraps our core library logging functionality provided by log4rs crate

use bevy::ecs::resource::Resource;

#[derive(Resource, Default)]
pub struct Logger {}
