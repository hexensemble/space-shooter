use macroquad::audio::{
    load_sound, play_sound, play_sound_once, set_sound_volume, PlaySoundParams, Sound,
};
use macroquad::experimental::animation::{AnimatedSprite, Animation};
use macroquad::experimental::collections::storage;
use macroquad::experimental::coroutines::start_coroutine;
use macroquad::prelude::*;
use macroquad::ui::{hash, root_ui, Skin};
use macroquad_particles::{self as particles, AtlasConfig, Emitter, EmitterConfig};
use std::fs;

// Shader
const FRAGMENT_SHADER: &str = include_str!("starfield-shader.glsl");

const VERTEX_SHADER: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying float iTime;

uniform mat4 Model;
uniform mat4 Projection;
uniform vec4 _Time;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    iTime = _Time.x;
}
";

#[macroquad::main("Space Shooter")]
async fn main() -> Result<(), macroquad::Error> {
    // Movement speed
    const MOVEMENT_SPEED: f32 = 200.0;

    // Use current date/time to generate random seed (used later to randomly generate enemies)
    rand::srand(miniquad::date::now() as u64);

    // Create Vecs for enemies, bullets, and explosions
    let mut enemies = vec![];
    let mut bullets: Vec<Shape> = vec![];
    let mut explosions: Vec<(Emitter, Vec2)> = vec![];

    // Create the player
    let mut player = Shape {
        size: 32.0,
        speed: MOVEMENT_SPEED,
        x: screen_width() / 2.0,
        y: screen_height() / 2.0,
        collided: false,
    };

    // Set initial game state to Main Menu
    let mut game_state = GameState::MainMenu;

    // Implement shader
    let mut direction_modifier: f32 = 0.0;
    let render_target = render_target(320, 150);
    render_target.texture.set_filter(FilterMode::Nearest);
    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: FRAGMENT_SHADER,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("iResolution", UniformType::Float2),
                UniformDesc::new("direction_modifier", UniformType::Float1),
            ],
            ..Default::default()
        },
    )?;

    // Initialize scores
    let mut score: u32 = 0;
    let mut high_score: u32 = fs::read_to_string("highscore.dat")
        .map_or(Ok(0), |i| i.parse::<u32>())
        .unwrap_or(0);

    // Initialize level
    let mut level: u32 = 1;

    // Set asset folder
    set_pc_assets_folder("assets");

    // Load resources
    Resources::load().await?;
    let resources = storage::get::<Resources>();

    // Create animations
    let mut enemy_small_sprite = AnimatedSprite::new(
        17,
        16,
        &[Animation {
            name: "enemy_small".to_string(),
            row: 0,
            frames: 2,
            fps: 12,
        }],
        true,
    );

    let mut enemy_medium_sprite = AnimatedSprite::new(
        32,
        16,
        &[Animation {
            name: "enemy_medium".to_string(),
            row: 0,
            frames: 2,
            fps: 12,
        }],
        true,
    );

    let mut enemy_large_sprite = AnimatedSprite::new(
        32,
        32,
        &[Animation {
            name: "enemy_large".to_string(),
            row: 0,
            frames: 2,
            fps: 12,
        }],
        true,
    );

    let mut bullet_sprite = AnimatedSprite::new(
        16,
        16,
        &[
            Animation {
                name: "bullet".to_string(),
                row: 0,
                frames: 2,
                fps: 12,
            },
            Animation {
                name: "bolt".to_string(),
                row: 1,
                frames: 2,
                fps: 12,
            },
        ],
        true,
    );

    let mut player_sprite = AnimatedSprite::new(
        16,
        24,
        &[
            Animation {
                name: "idle".to_string(),
                row: 0,
                frames: 2,
                fps: 12,
            },
            Animation {
                name: "left".to_string(),
                row: 2,
                frames: 2,
                fps: 12,
            },
            Animation {
                name: "right".to_string(),
                row: 4,
                frames: 2,
                fps: 12,
            },
        ],
        true,
    );

    // Play music
    play_sound(
        &resources.theme_music,
        PlaySoundParams {
            looped: true,
            volume: 0.5,
        },
    );

    // Set UI
    root_ui().push_skin(&resources.ui_skin);
    let window_size = vec2(370.0, 320.0);

    // Game loop
    loop {
        // Clear background and do shader stuff
        clear_background(BLACK);
        material.set_uniform("iResolution", (screen_width(), screen_height()));
        material.set_uniform("direction_modifier", direction_modifier);
        gl_use_material(&material);
        draw_texture_ex(
            &render_target.texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                ..Default::default()
            },
        );
        gl_use_default_material();

        // Game states
        match game_state {
            GameState::MainMenu => {
                // Create and display the menu. Set the game to init state when "Play" button is clicked
                root_ui().window(
                    hash!(),
                    vec2(
                        screen_width() / 2.0 - window_size.x / 2.0,
                        screen_height() / 2.0 - window_size.y / 2.0,
                    ),
                    window_size,
                    |ui| {
                        ui.label(vec2(80.0, -34.0), "Main Menu");
                        if ui.button(vec2(65.0, 25.0), "Play") {
                            enemies.clear();
                            bullets.clear();
                            explosions.clear();
                            player.x = screen_width() / 2.0;
                            player.y = screen_height() / 2.0;
                            score = 0;
                            level = 1;
                            game_state = GameState::Playing;
                        }
                        if ui.button(vec2(65.0, 125.0), "Quit") {
                            std::process::exit(0);
                        }
                    },
                );
            }

            GameState::Playing => {
                // Get delta time so frames stay consistent across devices
                let delta_time = get_frame_time();

                //Set player animation
                player_sprite.set_animation(0);

                // Handle keys
                if is_key_down(KeyCode::W) || is_key_down(KeyCode::K) {
                    player.y -= MOVEMENT_SPEED * delta_time;
                }
                if is_key_down(KeyCode::A) || is_key_down(KeyCode::H) {
                    player.x -= MOVEMENT_SPEED * delta_time;
                    direction_modifier -= 5.0 * delta_time;
                    player_sprite.set_animation(1);
                }
                if is_key_down(KeyCode::S) || is_key_down(KeyCode::J) {
                    player.y += MOVEMENT_SPEED * delta_time;
                }
                if is_key_down(KeyCode::D) || is_key_down(KeyCode::L) {
                    player.x += MOVEMENT_SPEED * delta_time;
                    direction_modifier += 5.0 * delta_time;
                    player_sprite.set_animation(2);
                }
                if is_key_pressed(KeyCode::Space) {
                    bullets.push(Shape {
                        size: 32.0,
                        speed: player.speed * 2.0,
                        x: player.x,
                        y: player.y - 24.0,
                        collided: false,
                    });
                    play_sound_once(&resources.sound_laser);
                }
                if is_key_pressed(KeyCode::Escape) {
                    game_state = GameState::Paused;
                }

                // Clamp X and Y so player stays within the screen
                player.x = clamp(player.x, 0.0, screen_width());
                player.y = clamp(player.y, 0.0, screen_height());

                // Random enemy generation
                if rand::gen_range(0, 99) >= 95 {
                    let size = rand::gen_range(16.0, 64.0);

                    let speed_modifier = level as f32 / 2.0;

                    enemies.push(Shape {
                        size,
                        speed: rand::gen_range(50.0 * speed_modifier, 150.0 * speed_modifier),
                        x: rand::gen_range(size / 2.0, screen_width() - size / 2.0),
                        y: -size,
                        collided: false,
                    });
                }

                // Enemy and bullet movement
                for enemy in &mut enemies {
                    enemy.y += enemy.speed * delta_time;
                }
                for bullet in &mut bullets {
                    bullet.y -= bullet.speed * delta_time;
                }

                // Update sprites
                enemy_small_sprite.update();
                enemy_medium_sprite.update();
                enemy_large_sprite.update();
                bullet_sprite.update();
                player_sprite.update();

                // Retain only entities inside the screen, discard others
                enemies.retain(|enemy| enemy.y < screen_height() + enemy.size);
                bullets.retain(|bullet| bullet.y > 0.0 - bullet.size / 2.0);

                // Retain only entities that haven't collided, discard others
                enemies.retain(|enemy| !enemy.collided);
                bullets.retain(|bullet| !bullet.collided);

                // Retain only explosions currently emitting, discard others
                explosions.retain(|(explosion, _)| explosion.config.emitting);

                //Check for bullet collisions
                for enemy in enemies.iter_mut() {
                    for bullet in bullets.iter_mut() {
                        if bullet.collides_with(enemy) {
                            bullet.collided = true;
                            enemy.collided = true;
                            score += enemy.size.round() as u32;

                            // Increase level every 1000 points (so enemy speed increases)
                            let new_level = score / 1000 + 1;
                            if new_level > level {
                                level = new_level;
                            }

                            high_score = high_score.max(score);
                            explosions.push((
                                Emitter::new(EmitterConfig {
                                    amount: enemy.size.round() as u32 * 4,
                                    texture: Some(resources.explosion_texture.clone()),
                                    ..particle_explosion()
                                }),
                                vec2(enemy.x, enemy.y),
                            ));
                            play_sound_once(&resources.sound_explosion);
                            set_sound_volume(&resources.sound_explosion, 0.4);
                        }
                    }
                }

                // Check for player collisions
                if enemies.iter().any(|enemy| player.collides_with(enemy)) {
                    if score == high_score {
                        fs::write("highscore.dat", high_score.to_string()).ok();
                    }
                    game_state = GameState::GameOver;
                }

                // Draw explosions
                for (explosion, coords) in explosions.iter_mut() {
                    explosion.draw(*coords);
                }

                // Draw enemies
                let enemy_small_frame = enemy_small_sprite.frame();
                let enemy_medium_frame = enemy_medium_sprite.frame();
                let enemy_large_frame = enemy_large_sprite.frame();
                for enemy in &enemies {
                    if enemy.size >= 16.0 && enemy.size < 32.0 {
                        draw_texture_ex(
                            &resources.enemy_small_texture,
                            enemy.x - enemy.size / 2.0,
                            enemy.y - enemy.size / 2.0,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(enemy.size, enemy.size)),
                                source: Some(enemy_small_frame.source_rect),
                                ..Default::default()
                            },
                        )
                    } else if enemy.size >= 32.0 && enemy.size < 48.0 {
                        draw_texture_ex(
                            &resources.enemy_medium_texture,
                            enemy.x - enemy.size / 2.0,
                            enemy.y - enemy.size / 2.0,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(enemy.size, enemy.size)),
                                source: Some(enemy_medium_frame.source_rect),
                                ..Default::default()
                            },
                        )
                    } else {
                        draw_texture_ex(
                            &resources.enemy_large_texture,
                            enemy.x - enemy.size / 2.0,
                            enemy.y - enemy.size / 2.0,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(enemy.size, enemy.size)),
                                source: Some(enemy_large_frame.source_rect),
                                ..Default::default()
                            },
                        )
                    }
                }

                // Draw bullets
                let bullet_frame = bullet_sprite.frame();
                for bullet in &bullets {
                    draw_texture_ex(
                        &resources.bullet_texture,
                        bullet.x - bullet.size / 2.0,
                        bullet.y - bullet.size,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(bullet.size, bullet.size)),
                            source: Some(bullet_frame.source_rect),
                            ..Default::default()
                        },
                    );
                }

                // Draw player
                let player_frame = player_sprite.frame();
                draw_texture_ex(
                    &resources.player_texture,
                    player.x - player_frame.dest_size.x,
                    player.y - player_frame.dest_size.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(player_frame.dest_size * 2.0),
                        source: Some(player_frame.source_rect),
                        ..Default::default()
                    },
                );

                // Draw scores
                draw_text(
                    format!("Score: {}", score).as_str(),
                    10.0,
                    35.0,
                    25.0,
                    WHITE,
                );

                let highscore_text = format!("High Score: {}", high_score);
                let text_dimensions = measure_text(highscore_text.as_str(), None, 25, 1.0);
                draw_text(
                    highscore_text.as_str(),
                    screen_width() - text_dimensions.width - 10.0,
                    35.0,
                    25.0,
                    WHITE,
                );
            }

            GameState::Paused => {
                // Press space to un-pause
                if is_key_pressed(KeyCode::Space) {
                    game_state = GameState::Playing;
                }

                // Display "Paused" text
                let text = "Paused";
                let text_dimensions = measure_text(text, None, 50, 1.0);
                draw_text(
                    text,
                    screen_width() / 2.0 - text_dimensions.width / 2.0,
                    screen_height() / 2.0,
                    50.0,
                    WHITE,
                );
            }

            GameState::GameOver => {
                // Press space to return to Main Menu
                if is_key_pressed(KeyCode::Space) {
                    game_state = GameState::MainMenu;
                }

                // Display "Game Over" text
                let text = "GAME OVER!";
                let text_dimensions = measure_text(text, None, 50, 1.0);
                draw_text(
                    text,
                    screen_width() / 2.0 - text_dimensions.width / 2.0,
                    screen_height() / 2.0,
                    50.0,
                    RED,
                );
            }
        }

        // Wait for frame to finish before we start the loop again
        next_frame().await;
    }
}

// Resources Struct
struct Resources {
    enemy_small_texture: Texture2D,
    enemy_medium_texture: Texture2D,
    enemy_large_texture: Texture2D,
    bullet_texture: Texture2D,
    explosion_texture: Texture2D,
    player_texture: Texture2D,
    theme_music: Sound,
    sound_explosion: Sound,
    sound_laser: Sound,
    ui_skin: Skin,
}

impl Resources {
    // New function
    async fn new() -> Result<Resources, macroquad::Error> {
        // Load texures
        let enemy_small_texture: Texture2D = load_texture("enemy-small.png").await?;
        enemy_small_texture.set_filter(FilterMode::Nearest);

        let enemy_medium_texture: Texture2D = load_texture("enemy-medium.png").await?;
        enemy_medium_texture.set_filter(FilterMode::Nearest);

        let enemy_large_texture: Texture2D = load_texture("enemy-large.png").await?;
        enemy_large_texture.set_filter(FilterMode::Nearest);

        let bullet_texture: Texture2D = load_texture("laser-bolts.png").await?;
        bullet_texture.set_filter(FilterMode::Nearest);

        let explosion_texture: Texture2D = load_texture("explosion.png").await?;
        explosion_texture.set_filter(FilterMode::Nearest);

        let player_texture: Texture2D = load_texture("player.png").await?;
        player_texture.set_filter(FilterMode::Nearest);

        build_textures_atlas();

        // Load audio
        let theme_music = load_sound("8bit-spaceshooter.ogg").await?;
        let sound_explosion = load_sound("explosion.wav").await?;
        let sound_laser = load_sound("laser.wav").await?;

        // Load UI
        let window_background = load_image("window_background.png").await?;
        let button_background = load_image("button_background.png").await?;
        let button_clicked_background = load_image("button_clicked_background.png").await?;
        let font = load_file("atari_games.ttf").await?;

        let window_style = root_ui()
            .style_builder()
            .background(window_background)
            .background_margin(RectOffset::new(32.0, 76.0, 44.0, 20.0))
            .margin(RectOffset::new(0.0, -40.0, 0.0, 0.0))
            .build();

        let button_style = root_ui()
            .style_builder()
            .background(button_background)
            .background_clicked(button_clicked_background)
            .background_margin(RectOffset::new(16.0, 16.0, 16.0, 16.0))
            .margin(RectOffset::new(16.0, 0.0, -8.0, -8.0))
            .font(&font)?
            .text_color(WHITE)
            .font_size(64)
            .build();

        let label_style = root_ui()
            .style_builder()
            .font(&font)?
            .text_color(WHITE)
            .font_size(28)
            .build();

        let ui_skin = Skin {
            window_style,
            button_style,
            label_style,
            ..root_ui().default_skin()
        };

        Ok(Resources {
            enemy_small_texture,
            enemy_medium_texture,
            enemy_large_texture,
            bullet_texture,
            explosion_texture,
            player_texture,
            theme_music,
            sound_explosion,
            sound_laser,
            ui_skin,
        })
    }

    // Load function - Displays loading screen on slower devices
    pub async fn load() -> Result<(), macroquad::Error> {
        let resources_loading = start_coroutine(async move {
            let resources = Resources::new().await.unwrap();
            storage::store(resources);
        });

        while !resources_loading.is_done() {
            clear_background(BLACK);
            let text = format!(
                "Loading resources {}",
                ".".repeat(((get_time() * 2.) as usize) % 4)
            );
            draw_text(
                &text,
                screen_width() / 2. - 160.,
                screen_height() / 2.,
                40.,
                WHITE,
            );
            next_frame().await;
        }

        Ok(())
    }
}

// Games states Enum
enum GameState {
    MainMenu,
    Playing,
    Paused,
    GameOver,
}

// Shape Struct
struct Shape {
    size: f32,
    speed: f32,
    x: f32,
    y: f32,
    collided: bool,
}

impl Shape {
    fn collides_with(&self, other: &Self) -> bool {
        self.rect().overlaps(&other.rect())
    }

    fn rect(&self) -> Rect {
        Rect {
            x: self.x - self.size / 2.0,
            y: self.y - self.size / 2.0,
            w: self.size,
            h: self.size,
        }
    }
}

// Explosions function
fn particle_explosion() -> particles::EmitterConfig {
    particles::EmitterConfig {
        local_coords: false,
        one_shot: true,
        emitting: true,
        lifetime: 0.6,
        lifetime_randomness: 0.3,
        explosiveness: 0.65,
        initial_direction_spread: 2.0 * std::f32::consts::PI,
        initial_velocity: 400.0,
        initial_velocity_randomness: 0.8,
        size: 16.0,
        size_randomness: 0.3,
        atlas: Some(AtlasConfig::new(5, 1, 0..)),
        ..Default::default()
    }
}
