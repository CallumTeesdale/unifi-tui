mod app;
mod error;
mod handlers;
mod state;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::{io, time::Duration};
use crossterm::event::MouseEvent;
use unifi_rs::UnifiClientBuilder;

use crate::app::{App, Mode};
use crate::handlers::{
    handle_client_detail_input, handle_device_detail_input, handle_dialog_input,
    handle_global_input, handle_search_input,
};
use crate::state::AppState;
use crate::ui::render;

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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
                                3 => ui::topology::handle_topology_input(&mut app, key).await?,
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
                                Constraint::Length(3),  // Title
                                Constraint::Min(0),     // Topology area
                                Constraint::Length(3),  // Status bar
                            ])
                            .split(area);

                        // Only handle mouse events in the topology area
                        if is_mouse_in_area(event, areas[1]) {
                            ui::topology::handle_topology_mouse(&mut app, event, areas[1]).await?;
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
