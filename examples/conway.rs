use std::mem;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use log::LevelFilter;
use num::Integer;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use valence::client::{Event, GameMode};
use valence::config::{Config, ServerListPing};
use valence::text::Color;
use valence::{
    async_trait, ident, Biome, BlockState, Client, Dimension, DimensionId, Server, ShutdownResult,
    Text, TextFormat, WorldId, Worlds,
};

pub fn main() -> ShutdownResult {
    env_logger::Builder::new()
        .filter_module("valence", LevelFilter::Trace)
        .parse_default_env()
        .init();

    valence::start_server(Game {
        player_count: AtomicUsize::new(0),
        state: Mutex::new(State {
            board: vec![false; SIZE_X * SIZE_Z].into_boxed_slice(),
            board_buf: vec![false; SIZE_X * SIZE_Z].into_boxed_slice(),
        }),
    })
}

struct Game {
    player_count: AtomicUsize,
    state: Mutex<State>,
}

struct State {
    board: Box<[bool]>,
    board_buf: Box<[bool]>,
}

const MAX_PLAYERS: usize = 10;

const SIZE_X: usize = 100;
const SIZE_Z: usize = 100;
const BOARD_Y: i32 = 50;

#[async_trait]
impl Config for Game {
    fn max_connections(&self) -> usize {
        // We want status pings to be successful even if the server is full.
        MAX_PLAYERS + 64
    }

    fn online_mode(&self) -> bool {
        // You'll want this to be true on real servers.
        false
    }

    fn biomes(&self) -> Vec<Biome> {
        vec![Biome {
            name: ident!("valence:default_biome"),
            grass_color: Some(0x00ff00),
            ..Biome::default()
        }]
    }

    fn dimensions(&self) -> Vec<Dimension> {
        vec![Dimension {
            fixed_time: Some(6000),
            ..Dimension::default()
        }]
    }

    async fn server_list_ping(&self, _server: &Server, _remote_addr: SocketAddr) -> ServerListPing {
        ServerListPing::Respond {
            online_players: self.player_count.load(Ordering::SeqCst) as i32,
            max_players: MAX_PLAYERS as i32,
            description: "Hello Valence!".color(Color::AQUA),
            favicon_png: Some(include_bytes!("favicon.png")),
        }
    }

    fn join(
        &self,
        _server: &Server,
        _client: &mut Client,
        worlds: &mut Worlds,
    ) -> Result<WorldId, Text> {
        if let Ok(_) = self
            .player_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                (count < MAX_PLAYERS).then(|| count + 1)
            })
        {
            Ok(worlds.iter().next().unwrap().0)
        } else {
            Err("The server is full!".into())
        }
    }

    fn init(&self, _server: &Server, worlds: &mut Worlds) {
        let world = worlds.create(DimensionId::default()).1;
        world.meta.set_flat(true);

        for chunk_z in -2..Integer::div_ceil(&(SIZE_X as i32), &16) + 2 {
            for chunk_x in -2..Integer::div_ceil(&(SIZE_Z as i32), &16) + 2 {
                world.chunks.create((chunk_x as i32, chunk_z as i32));
            }
        }
    }

    fn update(&self, server: &Server, worlds: &mut Worlds) {
        let world = worlds.iter_mut().next().unwrap().1;

        let spawn_pos = [
            SIZE_X as f64 / 2.0,
            BOARD_Y as f64 + 1.0,
            SIZE_Z as f64 / 2.0,
        ];

        world.clients.retain(|_, client| {
            if client.created_tick() == server.current_tick() {
                client.set_game_mode(GameMode::Survival);

                client.teleport(spawn_pos, 0.0, 0.0);

                world.meta.player_list_mut().insert(
                    client.uuid(),
                    client.username().to_string(),
                    client.textures().cloned(),
                    client.game_mode(),
                    0,
                    None,
                );

                client.send_message("Welcome to Conway's game of life in Minecraft!".italic());
                client.send_message("Hold the left mouse button to bring blocks to life.".italic());
            }

            if client.is_disconnected() {
                self.player_count.fetch_sub(1, Ordering::SeqCst);
                false
            } else {
                true
            }
        });

        let State { board, board_buf } = &mut *self.state.lock().unwrap();

        for (_, client) in world.clients.iter_mut() {
            while let Some(event) = client.pop_event() {
                match event {
                    Event::Digging(e) => {
                        let pos = e.position;

                        if (0..SIZE_X as i32).contains(&pos.x)
                            && (0..SIZE_Z as i32).contains(&pos.z)
                            && pos.y == BOARD_Y
                        {
                            board[pos.x as usize + pos.z as usize * SIZE_X] = true;
                        }
                    }
                    Event::Movement { position, .. } => {
                        if position.y <= 0.0 {
                            client.teleport(spawn_pos, client.pitch(), client.yaw());
                        }
                    }
                    _ => {}
                }
            }
        }

        if server.current_tick() % 4 != 0 {
            return;
        }

        board_buf.par_iter_mut().enumerate().for_each(|(i, cell)| {
            let cx = (i % SIZE_X) as i32;
            let cz = (i / SIZE_Z) as i32;

            let mut live_count = 0;
            for z in cz - 1..=cz + 1 {
                for x in cx - 1..=cx + 1 {
                    if !(x == cx && z == cz) {
                        let i = x.rem_euclid(SIZE_X as i32) as usize
                            + z.rem_euclid(SIZE_Z as i32) as usize * SIZE_X;
                        if board[i] {
                            live_count += 1;
                        }
                    }
                }
            }

            if board[cx as usize + cz as usize * SIZE_X] {
                *cell = (2..=3).contains(&live_count);
            } else {
                *cell = live_count == 3;
            }
        });

        mem::swap(board, board_buf);

        let min_y = server.dimensions().next().unwrap().1.min_y;

        for chunk_x in 0..Integer::div_ceil(&SIZE_X, &16) {
            for chunk_z in 0..Integer::div_ceil(&SIZE_Z, &16) {
                let chunk = world
                    .chunks
                    .get_mut((chunk_x as i32, chunk_z as i32))
                    .unwrap();
                for x in 0..16 {
                    for z in 0..16 {
                        let cell_x = chunk_x * 16 + x;
                        let cell_z = chunk_z * 16 + z;

                        if cell_x < SIZE_X && cell_z < SIZE_Z {
                            let b = if board[cell_x + cell_z * SIZE_X] {
                                BlockState::GRASS_BLOCK
                            } else {
                                BlockState::DIRT
                            };
                            chunk.set_block_state(x, (BOARD_Y - min_y) as usize, z, b);
                        }
                    }
                }
            }
        }
    }
}
