use bevy::prelude::*;
use clap::Parser;

#[derive(Parser, Resource, Debug, Clone)]
pub struct Args {
    /// runs the game in synctest mode
    #[clap(long)]
    pub synctest: bool,
}
