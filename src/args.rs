use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// runs the game in synctest mode
    #[clap(long)]
    pub synctest: bool,
}
