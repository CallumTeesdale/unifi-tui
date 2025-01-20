mod app;
mod error;
mod handlers;
mod state;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::event::MouseEvent;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use directories::ProjectDirs;
use log::LevelFilter;
use ratatui::prelude::*;
use simplelog::{CombinedLogger, ConfigBuilder, WriteLogger};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Once;
use std::{io, time::Duration};
use unifi_rs::UnifiClientBuilder;

use crate::app::{App, Mode};
use crate::handlers::{
    handle_client_detail_input, handle_device_detail_input, handle_dialog_input,
    handle_global_input, handle_search_input,
};
use crate::state::AppState;
use crate::ui::render;
use crate::ui::topology::topology::{handle_topology_input, handle_topology_mouse};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// UniFi Controller URL
    #[arg(long, env)]
    url: String,

    /// API Key
    #[arg(long, env)]
    api_key: String,

    /// Skip SSL verification
    #[arg(long, default_value = "false")]
    insecure: bool,
}

static INIT: Once = Once::new();

pub fn initialize_logging() -> Result<PathBuf, anyhow::Error> {
    let mut log_path = None;

    INIT.call_once(|| {
        if let Some(proj_dirs) = ProjectDirs::from("com", "unifi-tui", "unifi-tui") {
            let data_dir = proj_dirs.data_dir();
            std::fs::create_dir_all(data_dir).expect("Failed to create data directory");

            let log_file = data_dir.join("debug.log");
            log_path = Some(log_file.clone());

            let config = ConfigBuilder::new()
                .set_time_format_rfc3339()
                .set_thread_level(LevelFilter::Error)
                .set_target_level(LevelFilter::Error)
                .set_location_level(LevelFilter::Debug)
                .build();

            CombinedLogger::init(vec![WriteLogger::new(
                LevelFilter::Debug,
                config,
                File::create(&log_file).expect("Failed to create log file"),
            )])
            .expect("Failed to initialize logger");
        }
    });

    log_path.ok_or_else(|| anyhow::anyhow!("Failed to initialize logging"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let log_path = initialize_logging()?;
    log::info!("Starting application. Log file: {:?}", log_path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let client = UnifiClientBuilder::new(cli.url)
        .api_key(cli.api_key)
        .verify_ssl(!cli.insecure)
        .build()?;

    let state = AppState::new(client).await?;
    let app = App::new(state).await?;

    let res = run_app(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {err}");
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| render(&mut app, f))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_global_input(&mut app, key).await? {
                        continue;
                    }

                    if app.dialog.is_some() {
                        handle_dialog_input(&mut app, key).await?;
                    } else if app.search_mode {
                        handle_search_input(&mut app, key).await?;
                    } else if app.show_help {
                        if key.code == KeyCode::Esc {
                            app.show_help = false;
                        }
                    } else {
                        match app.mode {
                            Mode::Overview => match app.current_tab {
                                0 => ui::sites::handle_sites_input(&mut app, key)?,
                                1 => ui::devices::handle_device_input(&mut app, key).await?,
                                2 => ui::clients::handle_client_input(&mut app, key).await?,
                                3 => handle_topology_input(&mut app, key).await?,
                                4 => {}
                                _ => {}
                            },
                            Mode::DeviceDetail => {
                                handle_device_detail_input(&mut app, key).await?;
                            }
                            Mode::ClientDetail => {
                                handle_client_detail_input(&mut app, key).await?;
                            }
                            Mode::Help => {
                                if key.code == KeyCode::Esc {
                                    app.mode = Mode::Overview;
                                }
                            }
                        }
                    }
                }
                Event::Mouse(event) => {
                    if app.current_tab == 3 && app.mode == Mode::Overview {
                        // Get the terminal size and convert to Rect
                        let size = terminal.size()?;
                        let area = Rect::new(0, 0, size.width, size.height);

                        // Get the layout areas
                        let areas = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(3), // Title
                                Constraint::Min(0),    // Topology area
                                Constraint::Length(3), // Status bar
                            ])
                            .split(area);

                        // Only handle mouse events in the topology area
                        if is_mouse_in_area(event, areas[1]) {
                            handle_topology_mouse(&mut app, event, areas[1]).await?;
                        }
                    }
                }
                _ => {}
            }
        }

        if app.dialog.is_none() {
            if let Err(e) = app.refresh().await {
                app.state.set_error(format!("Error refreshing data: {}", e));
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
fn is_mouse_in_area(event: MouseEvent, area: Rect) -> bool {
    let (col, row) = (event.column, event.row);
    col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height
}
