use crate::plugins::init_game;

mod assets;
mod game_client;
mod plugins;

fn main() -> anyhow::Result<()> {
    init_game()
}
