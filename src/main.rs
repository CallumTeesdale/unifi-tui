use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use crossterm::event::KeyEvent;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::*,
};
use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};
use unifi_rs::{DeviceDetails, DeviceStatistics, UnifiClient, UnifiClientBuilder};

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

#[derive(PartialEq, Clone)]
enum Mode {
    Overview,
    DeviceDetail,
    ClientDetail,
    Help,
}

#[derive(PartialEq, Clone)]
enum DialogType {
    Confirmation,
    Message,
    Error,
}

#[derive(Clone, Copy)]
enum SortOrder {
    Ascending,
    Descending,
    None,
}

struct Dialog {
    title: String,
    message: String,
    dialog_type: DialogType,
    callback: Option<Box<dyn FnOnce(&mut App) -> Result<()> + Send>>,
}

#[derive(Clone)]
struct NetworkStats {
    timestamp: DateTime<Utc>,
    client_count: usize,
    wireless_clients: usize,
    wired_clients: usize,
    cpu_utilization: Vec<(String, f64)>,
    memory_utilization: Vec<(String, f64)>,
}

struct App {
    client: UnifiClient,
    sites: Vec<unifi_rs::SiteOverview>,
    devices: Vec<unifi_rs::DeviceOverview>,
    clients: Vec<unifi_rs::ClientOverview>,
    filtered_devices: Vec<unifi_rs::DeviceOverview>,
    filtered_clients: Vec<unifi_rs::ClientOverview>,
    current_tab: usize,
    selected_device_index: Option<usize>,
    selected_client_index: Option<usize>,
    device_details: Option<DeviceDetails>,
    device_stats: Option<DeviceStatistics>,
    stats_history: VecDeque<NetworkStats>,
    should_quit: bool,
    refresh_interval: Duration,
    last_update: Instant,
    mode: Mode,
    dialog: Option<Dialog>,
    search_mode: bool,
    search_query: String,
    error_message: Option<String>,
    error_timestamp: Option<Instant>,
    device_sort_column: usize,
    device_sort_order: SortOrder,
    client_sort_column: usize,
    client_sort_order: SortOrder,
    show_help: bool,
}

impl App {
    async fn new(url: String, api_key: String, insecure: bool) -> Result<App> {
        let client = UnifiClientBuilder::new(url)
            .api_key(api_key)
            .verify_ssl(!insecure)
            .build()?;

        Ok(App {
            client,
            sites: Vec::new(),
            devices: Vec::new(),
            clients: Vec::new(),
            filtered_devices: Vec::new(),
            filtered_clients: Vec::new(),
            current_tab: 0,
            selected_device_index: None,
            selected_client_index: None,
            device_details: None,
            device_stats: None,
            stats_history: VecDeque::with_capacity(100),
            should_quit: false,
            refresh_interval: Duration::from_secs(5),
            last_update: Instant::now(),
            mode: Mode::Overview,
            dialog: None,
            search_mode: false,
            search_query: String::new(),
            error_message: None,
            error_timestamp: None,
            device_sort_column: 0,
            device_sort_order: SortOrder::None,
            client_sort_column: 0,
            client_sort_order: SortOrder::None,
            show_help: false,
        })
    }

    async fn refresh_data(&mut self) -> Result<()> {
        if self.last_update.elapsed() >= self.refresh_interval {
            let sites = self.client.list_sites(None, None).await?;
            self.sites = sites.data;

            if let Some(site) = self.sites.first() {
                let devices = self.client.list_devices(site.id, None, None).await?;
                self.devices = devices.data;
                self.filtered_devices = self.devices.clone();

                let clients = self.client.list_clients(site.id, None, None).await?;
                self.clients = clients.data;
                self.filtered_clients = self.clients.clone();

                if let Some(idx) = self.selected_device_index {
                    if let Some(device) = self.devices.get(idx) {
                        self.device_details = self
                            .client
                            .get_device_details(site.id, device.id)
                            .await
                            .ok();
                        self.device_stats = self
                            .client
                            .get_device_statistics(site.id, device.id)
                            .await
                            .ok();
                    }
                }

                let stats = NetworkStats {
                    timestamp: Utc::now(),
                    client_count: self.clients.len(),
                    wireless_clients: self
                        .clients
                        .iter()
                        .filter(|c| matches!(c, unifi_rs::ClientOverview::Wireless(_)))
                        .count(),
                    wired_clients: self
                        .clients
                        .iter()
                        .filter(|c| matches!(c, unifi_rs::ClientOverview::Wired(_)))
                        .count(),
                    cpu_utilization: Vec::new(),
                    memory_utilization: Vec::new(),
                };

                if self.stats_history.len() >= 100 {
                    self.stats_history.pop_front();
                }
                self.stats_history.push_back(stats);
            }

            self.last_update = Instant::now();

            self.sort_devices();
            self.sort_clients();

            if !self.search_query.is_empty() {
                self.apply_filters();
            }
        }
        Ok(())
    }

    fn sort_devices(&mut self) {
        if let SortOrder::None = self.device_sort_order {
            return;
        }

        self.filtered_devices.sort_by(|a, b| {
            let cmp = match self.device_sort_column {
                0 => a.name.cmp(&b.name),
                1 => a.model.cmp(&b.model),
                2 => a.mac_address.cmp(&b.mac_address),
                3 => a.ip_address.cmp(&b.ip_address),
                4 => format!("{:?}", a.state).cmp(&format!("{:?}", b.state)), // TODO: Change DeviceState to implement to string
                _ => std::cmp::Ordering::Equal,
            };
            match self.device_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
                SortOrder::None => cmp,
            }
        });
    }

    fn sort_clients(&mut self) {
        if let SortOrder::None = self.client_sort_order {
            return;
        }

        self.filtered_clients.sort_by(|a, b| {
            let (a_name, a_ip, a_mac) = match a {
                unifi_rs::ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                unifi_rs::ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                _ => ("", "", ""),
            };

            let (b_name, b_ip, b_mac) = match b {
                unifi_rs::ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                unifi_rs::ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                _ => ("", "", ""),
            };

            let cmp = match self.client_sort_column {
                0 => a_name.cmp(b_name),
                1 => a_ip.cmp(b_ip),
                2 => a_mac.cmp(b_mac),
                _ => std::cmp::Ordering::Equal,
            };
            match self.client_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
                SortOrder::None => cmp,
            }
        });
    }

    fn apply_filters(&mut self) {
        let query = self.search_query.to_lowercase();

        self.filtered_devices = self
            .devices
            .iter()
            .filter(|d| {
                d.name.to_lowercase().contains(&query)
                    || d.model.to_lowercase().contains(&query)
                    || d.mac_address.to_lowercase().contains(&query)
                    || d.ip_address.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        self.filtered_clients = self
            .clients
            .iter()
            .filter(|c| match c {
                unifi_rs::ClientOverview::Wired(wc) => {
                    wc.base
                        .name
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                        || wc
                            .base
                            .ip_address
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || wc.mac_address.to_lowercase().contains(&query)
                }
                unifi_rs::ClientOverview::Wireless(wc) => {
                    wc.base
                        .name
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                        || wc
                            .base
                            .ip_address
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || wc.mac_address.to_lowercase().contains(&query)
                }
                _ => false,
            })
            .cloned()
            .collect();
    }

    fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.error_timestamp = Some(Instant::now());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(cli.url, cli.api_key, cli.insecure).await?;
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

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let menu_titles = vec!["Sites", "Devices", "Clients", "Stats"];

    loop {
        if app.dialog.is_none() {
            if let Err(e) = app.refresh_data().await {
                app.set_error(format!("Error refreshing data: {}", e));
            }
        }

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
                .split(size);

            if let Some(dialog) = &app.dialog {
                draw_dialog(f, chunks[0], dialog);
            } else if app.show_help {
                draw_help(f, chunks[0]);
            } else {
                match app.mode {
                    Mode::Overview => draw_overview(f, chunks[0], &app, &menu_titles),
                    Mode::DeviceDetail => draw_device_detail(f, chunks[0], &app),
                    Mode::ClientDetail => draw_client_detail(f, chunks[0], &app),
                    Mode::Help => draw_help(f, chunks[0]),
                }
            }

            draw_status_bar(f, chunks[1], &app);

            if let Some(error) = &app.error_message {
                if let Some(timestamp) = app.error_timestamp {
                    if timestamp.elapsed() < Duration::from_secs(5) {
                        draw_error(f, size, error);
                    } else {
                        app.error_message = None;
                        app.error_timestamp = None;
                    }
                }
            }

            if app.search_mode {
                draw_search_bar(f, size, &app);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let Some(dialog) = &app.dialog {
                    match key.code {
                        KeyCode::Char('y') if dialog.dialog_type == DialogType::Confirmation => {
                            if let Some(callback) = app.dialog.take().and_then(|d| d.callback) {
                                if let Err(e) = callback(&mut app) {
                                    app.set_error(format!("Operation failed: {}", e));
                                }
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.dialog = None,
                        _ => {}
                    }
                    continue;
                }

                if app.search_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.search_mode = false;
                            app.search_query.clear();
                            app.filtered_devices = app.devices.clone();
                            app.filtered_clients = app.clients.clone();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            app.apply_filters();
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            app.apply_filters();
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.show_help {
                    if key.code == KeyCode::Esc {
                        app.show_help = false;
                    }
                    continue;
                }

                match app.mode {
                    Mode::Overview => handle_overview_input(&mut app, key).await?,
                    Mode::DeviceDetail => handle_detail_input(&mut app, key).await?,
                    Mode::ClientDetail => handle_client_input(&mut app, key).await?,
                    Mode::Help => {
                        if key.code == KeyCode::Esc {
                            app.mode = Mode::Overview;
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_overview_input(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => app.show_help = !app.show_help,
        KeyCode::Char('/') => {
            app.search_mode = true;
            app.search_query.clear();
        }
        KeyCode::Tab => app.current_tab = (app.current_tab + 1) % 4,
        KeyCode::BackTab => app.current_tab = (app.current_tab + 3) % 4,
        KeyCode::Char('r') => app.last_update = Instant::now() - app.refresh_interval,
        KeyCode::Char('s') => match app.current_tab {
            1 => {
                match app.device_sort_order {
                    SortOrder::None => app.device_sort_order = SortOrder::Ascending,
                    SortOrder::Ascending => app.device_sort_order = SortOrder::Descending,
                    SortOrder::Descending => app.device_sort_order = SortOrder::None,
                }
                app.sort_devices();
            }
            2 => {
                match app.client_sort_order {
                    SortOrder::None => app.client_sort_order = SortOrder::Ascending,
                    SortOrder::Ascending => app.client_sort_order = SortOrder::Descending,
                    SortOrder::Descending => app.client_sort_order = SortOrder::None,
                }
                app.sort_clients();
            }
            _ => {}
        },
        KeyCode::Down => match app.current_tab {
            1 => {
                // Devices tab
                if let Some(idx) = app.selected_device_index {
                    app.selected_device_index =
                        Some((idx + 1).min(app.filtered_devices.len().saturating_sub(1)));
                } else if !app.filtered_devices.is_empty() {
                    app.selected_device_index = Some(0);
                }
            }
            2 => {
                // Clients tab
                if let Some(idx) = app.selected_client_index {
                    app.selected_client_index =
                        Some((idx + 1).min(app.filtered_clients.len().saturating_sub(1)));
                } else if !app.filtered_clients.is_empty() {
                    app.selected_client_index = Some(0);
                }
            }
            _ => {}
        },
        KeyCode::Up => match app.current_tab {
            1 => {
                // Devices tab
                if let Some(idx) = app.selected_device_index {
                    app.selected_device_index = Some(idx.saturating_sub(1));
                }
            }
            2 => {
                // Clients tab
                if let Some(idx) = app.selected_client_index {
                    app.selected_client_index = Some(idx.saturating_sub(1));
                }
            }
            _ => {}
        },
        KeyCode::Enter => match app.current_tab {
            1 if app.selected_device_index.is_some() => {
                app.mode = Mode::DeviceDetail;
            }
            2 if app.selected_client_index.is_some() => {
                app.mode = Mode::ClientDetail;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}
async fn handle_detail_input(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Overview;
            app.device_details = None;
            app.device_stats = None;
        }
        KeyCode::Char('r') => {
            if let Some(idx) = app.selected_device_index {
                if let Some(device) = app.devices.get(idx) {
                    if let Some(site) = app.sites.first() {
                        let device_id = device.id;
                        let site_id = site.id;
                        let client = app.client.clone();

                        app.dialog = Some(Dialog {
                            title: "Confirm Restart".to_string(),
                            message: format!(
                                "Are you sure you want to restart {}? (y/n)",
                                device.name
                            ),
                            dialog_type: DialogType::Confirmation,
                            callback: Some(Box::new(move |_app| {
                                let result = tokio::runtime::Handle::current().block_on(async {
                                    client.restart_device(site_id, device_id).await
                                });
                                result.map_err(anyhow::Error::from)
                            })),
                        });
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_client_input(app: &mut App, key: KeyEvent) -> io::Result<()> {
    if key.code == KeyCode::Esc {
        app.mode = Mode::Overview;
    }
    Ok(())
}
fn draw_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let area = centered_rect(60, 3, area);
    let search_widget = Paragraph::new(format!("Search: {}", app.search_query))
        .block(Block::default().borders(Borders::ALL).title("Search"))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(Clear, area);
    f.render_widget(search_widget, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let status = format!(
        " Sites: {} | Devices: {} | Clients: {} | Last Update: {}s ago {}",
        app.sites.len(),
        app.devices.len(),
        app.clients.len(),
        app.last_update.elapsed().as_secs(),
        if app.search_mode { "| SEARCH MODE" } else { "" }
    );

    let status_widget = Paragraph::new(Line::from(status))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    f.render_widget(status_widget, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from("UniFi Network TUI Help"),
        Line::from(""),
        Line::from("Global Commands:"),
        Line::from("  q      - Quit"),
        Line::from("  ?      - Toggle help"),
        Line::from("  /      - Search"),
        Line::from("  Tab    - Switch views"),
        Line::from("  r      - Refresh data"),
        Line::from(""),
        Line::from("Device/Client List:"),
        Line::from("  ↑/↓    - Navigate"),
        Line::from("  Enter  - View details"),
        Line::from("  s      - Toggle sort"),
        Line::from(""),
        Line::from("Device Details:"),
        Line::from("  r      - Restart device"),
        Line::from("  Esc    - Back to list"),
        Line::from(""),
        Line::from("Search Mode:"),
        Line::from("  Type   - Filter items"),
        Line::from("  Esc    - Exit search"),
    ];

    let help_widget = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .alignment(Alignment::Left);

    f.render_widget(help_widget, area);
}

fn draw_error(f: &mut Frame, area: Rect, error: &str) {
    let area = centered_rect(60, 3, area);
    let error_widget = Paragraph::new(error)
        .block(Block::default().borders(Borders::ALL).title("Error"))
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center);

    f.render_widget(Clear, area);
    f.render_widget(error_widget, area);
}

fn draw_overview(f: &mut Frame, area: Rect, app: &App, menu_titles: &[&str]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(area);

    let menu = menu_titles
        .iter()
        .map(|t| Line::from(*t))
        .collect::<Vec<Line>>();

    let tabs = Tabs::new(menu)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.current_tab)
        .style(Style::default())
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Gray),
        );
    f.render_widget(tabs, chunks[0]);

    match app.current_tab {
        0 => render_sites(f, chunks[1], app),
        1 => render_devices(f, chunks[1], app),
        2 => render_clients(f, chunks[1], app),
        3 => render_stats(f, chunks[1], app),
        _ => {}
    }

    let help_text = match app.current_tab {
        0 => vec![Line::from(
            "q: quit | ?: help | tab: switch views | r: refresh",
        )],
        1 => vec![Line::from(
            "q: quit | ?: help | ↑↓: select | enter: details | s: sort | /: search",
        )],
        2 => vec![Line::from(
            "q: quit | ?: help | ↑↓: select | enter: details | s: sort | /: search",
        )],
        3 => vec![Line::from(
            "q: quit | ?: help | r: refresh | tab: switch views",
        )],
        _ => vec![],
    };

    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[2]);
}

fn render_sites(f: &mut Frame, area: Rect, app: &App) {
    let sites: Vec<Row> = app
        .sites
        .iter()
        .map(|site| {
            let cells = vec![
                Cell::from(site.id.to_string()),
                Cell::from(site.name.as_deref().unwrap_or("Unnamed")),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = Table::new(sites, widths)
        .header(Row::new(vec!["ID", "Name"]))
        .block(Block::default().borders(Borders::ALL).title("Sites"))
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("> ");

    f.render_widget(table, area);
}
fn render_devices(f: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default().bg(Color::Gray);

    let devices: Vec<Row> = app
        .filtered_devices
        .iter()
        .enumerate()
        .map(|(idx, device)| {
            let style = if Some(idx) == app.selected_device_index {
                selected_style
            } else {
                Style::default()
            };

            let state_style = match device.state {
                unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
                unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Yellow),
            };

            let cells = vec![
                Cell::from(device.name.clone()),
                Cell::from(device.model.clone()),
                Cell::from(device.mac_address.clone()),
                Cell::from(device.ip_address.clone()),
                Cell::from(format!("{:?}", device.state)).style(state_style),
                Cell::from(device.features.join(", ")),
            ];
            Row::new(cells).style(style)
        })
        .collect();

    let header_cells = vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Features").style(Style::default().add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
    ];

    let table = Table::new(devices, widths)
        .header(Row::new(header_cells))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Devices ({}) [{}]",
            app.filtered_devices.len(),
            match app.device_sort_order {
                SortOrder::Ascending => "↑",
                SortOrder::Descending => "↓",
                SortOrder::None => "-",
            }
        )))
        .row_highlight_style(selected_style)
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

fn render_clients(f: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default().bg(Color::Gray);

    let clients: Vec<Row> = app
        .filtered_clients
        .iter()
        .enumerate()
        .map(|(idx, client)| {
            let style = if Some(idx) == app.selected_client_index {
                selected_style
            } else {
                Style::default()
            };

            let (name, ip, mac, r#type, connected_since, status) = match client {
                unifi_rs::ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    c.mac_address.clone(),
                    Cell::from("Wired").style(Style::default().fg(Color::Blue)),
                    c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Cell::from("Connected").style(Style::default().fg(Color::Green)),
                ),
                unifi_rs::ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    c.mac_address.clone(),
                    Cell::from("Wireless").style(Style::default().fg(Color::Yellow)),
                    c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Cell::from("Connected").style(Style::default().fg(Color::Green)),
                ),
                _ => (
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    Cell::from("Other").style(Style::default().fg(Color::Red)),
                    "Unknown".to_string(),
                    Cell::from("Unknown").style(Style::default().fg(Color::Red)),
                ),
            };

            Row::new(vec![
                Cell::from(name),
                Cell::from(ip),
                Cell::from(mac),
                r#type,
                Cell::from(connected_since),
                status,
            ])
            .style(style)
        })
        .collect();

    let header_cells = vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Connected Since").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
    ];

    let table = Table::new(clients, widths)
        .header(Row::new(header_cells))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Clients ({}) [{}]",
            app.filtered_clients.len(),
            match app.client_sort_order {
                SortOrder::Ascending => "↑",
                SortOrder::Descending => "↓",
                SortOrder::None => "-",
            }
        )))
        .row_highlight_style(selected_style)
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(area);

    let summary_text = vec![
        Line::from(format!("Total Sites: {}", app.sites.len())),
        Line::from(format!(
            "Total Devices: {} ({} online)",
            app.devices.len(),
            app.devices
                .iter()
                .filter(|d| matches!(d.state, unifi_rs::DeviceState::Online))
                .count()
        )),
        Line::from(format!("Total Clients: {}", app.clients.len())),
        Line::from(format!(
            "Wireless Clients: {}",
            app.clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wireless(_)))
                .count()
        )),
        Line::from(format!(
            "Wired Clients: {}",
            app.clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wired(_)))
                .count()
        )),
    ];

    let summary = Paragraph::new(summary_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Network Summary"),
        )
        .style(Style::default());
    f.render_widget(summary, chunks[0]);

    let client_history: Vec<&NetworkStats> = app.stats_history.iter().collect();
    if !client_history.is_empty() {
        let total_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.client_count as f64))
            .collect();

        let wireless_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wireless_clients as f64))
            .collect();

        let wired_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wired_clients as f64))
            .collect();

        let max_y = client_history
            .iter()
            .map(|s| s.client_count as f64)
            .fold(0.0, f64::max);

        let datasets = vec![
            Dataset::default()
                .name("Total")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&total_data),
            Dataset::default()
                .name("Wireless")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&wireless_data),
            Dataset::default()
                .name("Wired")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Blue))
                .data(&wired_data),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title("Client History")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title(Span::styled("Time", Style::default().fg(Color::Gray)))
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, (client_history.len() - 1) as f64])
                    .labels(
                        vec![
                            Span::styled("5m ago", Style::default().fg(Color::Gray)),
                            Span::styled("Now", Style::default().fg(Color::Gray)),
                        ]
                        .into_iter()
                        .map(Line::from)
                        .collect::<Vec<Line>>(),
                    ),
            )
            .y_axis(
                Axis::default()
                    .title(Span::styled("Clients", Style::default().fg(Color::Gray)))
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_y * 1.1])
                    .labels(
                        vec![
                            Span::styled("0", Style::default().fg(Color::Gray)),
                            Span::styled(
                                format!("{}", max_y as i32),
                                Style::default().fg(Color::Gray),
                            ),
                        ]
                        .into_iter()
                        .map(Line::from)
                        .collect::<Vec<Line>>(),
                    ),
            );

        f.render_widget(chart, chunks[1]);
    }
}

fn draw_device_detail(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let selected_device = app
        .selected_device_index
        .and_then(|idx| app.filtered_devices.get(idx));

    if let Some(device) = selected_device {
        let state_style = match device.state {
            unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
            unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
            _ => Style::default().fg(Color::Yellow),
        };

        let mut details_text = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Model: "),
                Span::styled(&device.model, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("MAC: "),
                Span::styled(
                    &device.mac_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("IP: "),
                Span::styled(
                    &device.ip_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("State: "),
                Span::styled(format!("{:?}", device.state), state_style),
            ]),
            Line::from(vec![
                Span::raw("Features: "),
                Span::styled(device.features.join(", "), Style::default()),
            ]),
            Line::from(""),
        ];

        if let Some(stats) = &app.device_stats {
            details_text.extend(vec![
                Line::from(format!("Uptime: {} hours", stats.uptime_sec / 3600)),
                Line::from(format!(
                    "CPU: {}%",
                    stats.cpu_utilization_pct.unwrap_or(0.0)
                )),
                Line::from(format!(
                    "Memory: {}%",
                    stats.memory_utilization_pct.unwrap_or(0.0)
                )),
            ]);
        }

        let content = Paragraph::new(details_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Device Details"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[0]);
    }

    let help_text = vec![Line::from("ESC: Back | r: Restart Device | q: Quit")];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
}

fn draw_client_detail(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let selected_client = app
        .selected_client_index
        .and_then(|idx| app.filtered_clients.get(idx));

    if let Some(client) = selected_client {
        let details_text = match client {
            unifi_rs::ClientOverview::Wired(c) => vec![
                Line::from(vec![
                    Span::raw("Name: "),
                    Span::styled(
                        c.base.name.as_deref().unwrap_or("Unnamed"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Type: "),
                    Span::styled("Wired", Style::default().fg(Color::Blue)),
                ]),
                Line::from(vec![
                    Span::raw("MAC: "),
                    Span::styled(
                        &c.mac_address,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("IP: "),
                    Span::styled(
                        c.base.ip_address.as_deref().unwrap_or("Unknown"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Connected Since: "),
                    Span::styled(
                        c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        Style::default(),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Uplink Device: "),
                    Span::styled(c.uplink_device_id.to_string(), Style::default()),
                ]),
            ],
            unifi_rs::ClientOverview::Wireless(c) => vec![
                Line::from(vec![
                    Span::raw("Name: "),
                    Span::styled(
                        c.base.name.as_deref().unwrap_or("Unnamed"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Type: "),
                    Span::styled("Wireless", Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::raw("MAC: "),
                    Span::styled(
                        &c.mac_address,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("IP: "),
                    Span::styled(
                        c.base.ip_address.as_deref().unwrap_or("Unknown"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Connected Since: "),
                    Span::styled(
                        c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        Style::default(),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Uplink Device: "),
                    Span::styled(c.uplink_device_id.to_string(), Style::default()),
                ]),
            ],
            _ => vec![Line::from("Unknown client type")],
        };

        let content = Paragraph::new(details_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Client Details"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[0]);
    }

    let help_text = vec![Line::from("ESC: Back | q: Quit")];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
}

fn draw_dialog(f: &mut Frame, area: Rect, dialog: &Dialog) {
    let area = centered_rect(60, 20, area);
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(dialog.message.clone()),
        Line::from(""),
        Line::from(match dialog.dialog_type {
            DialogType::Confirmation => "(y) Yes  (n) No",
            DialogType::Message | DialogType::Error => "Press any key to close",
        }),
    ];

    let dialog_style = match dialog.dialog_type {
        DialogType::Error => Style::default().fg(Color::Red),
        _ => Style::default(),
    };

    let dialog_box = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(dialog.title.as_str()),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(dialog_style);
    f.render_widget(dialog_box, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
