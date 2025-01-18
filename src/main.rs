use std::{collections::VecDeque, io, time::{Duration, Instant}};
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::*,
    style::{Color, Style},
};
use unifi_rs::{UnifiClient, UnifiClientBuilder, DeviceDetails, DeviceStatistics};

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
}

#[derive(PartialEq, Clone)]
enum DialogType {
    Confirmation,
    Message,
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
    current_tab: usize,
    selected_device_index: Option<usize>,
    device_details: Option<DeviceDetails>,
    device_stats: Option<DeviceStatistics>,
    stats_history: VecDeque<NetworkStats>,
    should_quit: bool,
    refresh_interval: Duration,
    last_update: Instant,
    mode: Mode,
    dialog: Option<Dialog>,
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
            current_tab: 0,
            selected_device_index: None,
            device_details: None,
            device_stats: None,
            stats_history: VecDeque::with_capacity(100),
            should_quit: false,
            refresh_interval: Duration::from_secs(5),
            last_update: Instant::now(),
            mode: Mode::Overview,
            dialog: None,
        })
    }

    async fn refresh_data(&mut self) -> Result<()> {
        if self.last_update.elapsed() >= self.refresh_interval {
            let sites = self.client.list_sites(None, None).await?;
            self.sites = sites.data;

            if let Some(site) = self.sites.first() {
                let devices = self.client.list_devices(site.id, None, None).await?;
                self.devices = devices.data;

                let clients = self.client.list_clients(site.id, None, None).await?;
                self.clients = clients.data;

                // Update selected device details if any
                if let Some(idx) = self.selected_device_index {
                    if let Some(device) = self.devices.get(idx) {
                        self.device_details = self.client.get_device_details(site.id, device.id).await.ok();
                        self.device_stats = self.client.get_device_statistics(site.id, device.id).await.ok();
                    }
                }

                // Update network stats
                let stats = NetworkStats {
                    timestamp: Utc::now(),
                    client_count: self.clients.len(),
                    wireless_clients: self.clients.iter().filter(|c| matches!(c, unifi_rs::ClientOverview::Wireless(_))).count(),
                    wired_clients: self.clients.iter().filter(|c| matches!(c, unifi_rs::ClientOverview::Wired(_))).count(),
                    cpu_utilization: Vec::new(), // TODO: Populate from device stats
                    memory_utilization: Vec::new(),
                };

                if self.stats_history.len() >= 100 {
                    self.stats_history.pop_front();
                }
                self.stats_history.push_back(stats);
            }

            self.last_update = Instant::now();
        }
        Ok(())
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
                eprintln!("Error refreshing data: {}", e);
            }
        }

        terminal.draw(|f| {
            let size = f.area();

            if let Some(dialog) = &app.dialog {
                draw_dialog::<B>(f, size, dialog);
                return;
            }

            match app.mode {
                Mode::Overview => draw_overview::<B>(f, size, &app, &menu_titles),
                Mode::DeviceDetail => draw_device_detail::<B>(f, size, &app),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let Some(dialog) = &app.dialog {
                    match key.code {
                        KeyCode::Char('y') if dialog.dialog_type == DialogType::Confirmation => {
                            if let Some(callback) = app.dialog.take().and_then(|d| d.callback) {
                                if let Err(e) = callback(&mut app) {
                                    app.dialog = Some(Dialog {
                                        title: "Error".to_string(),
                                        message: format!("Operation failed: {}", e),
                                        dialog_type: DialogType::Message,
                                        callback: None,
                                    });
                                }
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.dialog = None,
                        _ => {}
                    }
                    continue;
                }

                match app.mode {
                    Mode::Overview => handle_overview_input(&mut app, key.code).await?,
                    Mode::DeviceDetail => handle_detail_input(&mut app, key.code).await?,
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_overview_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.current_tab = (app.current_tab + 1) % 4,
        KeyCode::BackTab => app.current_tab = (app.current_tab + 3) % 4,
        KeyCode::Char('r') => app.last_update = Instant::now() - app.refresh_interval,
        KeyCode::Down if app.current_tab == 1 => {
            if let Some(idx) = app.selected_device_index {
                app.selected_device_index = Some((idx + 1).min(app.devices.len().saturating_sub(1)));
            } else if !app.devices.is_empty() {
                app.selected_device_index = Some(0);
            }
        }
        KeyCode::Up if app.current_tab == 1 => {
            if let Some(idx) = app.selected_device_index {
                app.selected_device_index = Some(idx.saturating_sub(1));
            }
        }
        KeyCode::Enter if app.current_tab == 1 && app.selected_device_index.is_some() => {
            app.mode = Mode::DeviceDetail;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_detail_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
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
                            message: format!("Are you sure you want to restart {}? (y/n)", device.name),
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

fn draw_overview<B: Backend>(f: &mut Frame, area: Rect, app: &App, menu_titles: &[&str]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ].as_ref())
        .split(area);

    let menu = menu_titles
        .iter()
        .map(|t| Line::from(*t))
        .collect::<Vec<Line>>();
    let tabs = Tabs::new(menu)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.current_tab)
        .style(Style::default())
        .highlight_style(Style::default().bg(Color::Gray));
    f.render_widget(tabs, chunks[0]);

    match app.current_tab {
        0 => render_sites::<B>(f, chunks[1], app),
        1 => render_devices::<B>(f, chunks[1], app),
        2 => render_clients(f, chunks[1], app),
        3 => render_stats::<B>(f, chunks[1], app),
        _ => {}
    }

    let help_text = match app.current_tab {
        1 => vec![
            Line::from("Press 'q' to quit"),
            Line::from("Press 'tab' to switch views"),
            Line::from("Up/Down to select device, Enter for details"),
        ],
        _ => vec![
            Line::from("Press 'q' to quit"),
            Line::from("Press 'tab' to switch views"),
            Line::from("Press 'r' to refresh data"),
        ],
    };
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[2]);
}

fn draw_device_detail<B: Backend>(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
        ].as_ref())
        .split(area);

    let selected_device = app.selected_device_index.and_then(|idx| app.devices.get(idx));

    if let Some(device) = selected_device {
        let details_text = vec![
            Line::from(format!("Name: {}", device.name)),
            Line::from(format!("Model: {}", device.model)),
            Line::from(format!("MAC: {}", device.mac_address)),
            Line::from(format!("IP: {}", device.ip_address)),
            Line::from(format!("State: {:?}", device.state)),
            Line::from(""),
        ];

        if let Some(stats) = &app.device_stats {
            let mut stats_text = details_text;
            stats_text.extend(vec![
                Line::from(format!("Uptime: {} hours", stats.uptime_sec / 3600)),
                Line::from(format!("CPU: {}%", stats.cpu_utilization_pct.unwrap_or(0.0))),
                Line::from(format!("Memory: {}%", stats.memory_utilization_pct.unwrap_or(0.0))),
            ]);

            let content = Paragraph::new(stats_text)
                .block(Block::default().borders(Borders::ALL).title("Device Details"))
                .wrap(Wrap { trim: true });
            f.render_widget(content, chunks[0]);
        }
    }

    let help_text = vec![
        Line::from("Press 'Esc' to go back"),
        Line::from("Press 'r' to restart device"),
        Line::from("Press 'q' to quit"),
    ];
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[1]);
}

fn draw_dialog<B: Backend>(f: &mut Frame, area: Rect, dialog: &Dialog) {
    let area = centered_rect(60, 20, area);
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(dialog.message.clone()),
        Line::from(""),
        Line::from(match dialog.dialog_type {
            DialogType::Confirmation => "(y) Yes  (n) No",
            DialogType::Message => "Press any key to close",
        }),
    ];

    let dialog_box = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(dialog.title.as_str()))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(dialog_box, area);
}

fn render_sites<B: Backend>(f: &mut Frame, area: Rect, app: &App) {
    let sites: Vec<Row> = app.sites.iter().map(|site| {
        let cells = vec![
            Cell::from(site.id.to_string()),
            Cell::from(site.name.as_deref().unwrap_or("Unnamed")),
        ];
        Row::new(cells)
    }).collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ];

    let table = Table::new(sites, widths)
        .header(Row::new(vec!["ID", "Name"]))
        .block(Block::default().borders(Borders::ALL).title("Sites"))
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

fn render_devices<B: Backend>(f: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default().bg(Color::Gray);

    let devices: Vec<Row> = app.devices.iter().enumerate().map(|(idx, device)| {
        let style = if Some(idx) == app.selected_device_index {
            selected_style
        } else {
            Style::default()
        };

        let cells = vec![
            Cell::from(device.name.clone()),
            Cell::from(device.model.clone()),
            Cell::from(device.mac_address.clone()),
            Cell::from(device.ip_address.clone()),
            Cell::from(format!("{:?}", device.state)),
        ];
        Row::new(cells).style(style)
    }).collect();

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];

    let table = Table::new(devices, widths)
        .header(Row::new(vec!["Name", "Model", "MAC", "IP", "State"]))
        .block(Block::default().borders(Borders::ALL).title("Devices"))
        .row_highlight_style(selected_style)
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

fn render_clients(f: &mut Frame, area: Rect, app: &App) {
    let clients: Vec<Row> = app.clients.iter().map(|client| {
        let (name, ip, mac, r#type) = match client {
            unifi_rs::ClientOverview::Wired(c) => (
                c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                c.base.ip_address.as_deref().unwrap_or("Unknown").to_string(),
                c.mac_address.clone(),
                "Wired".to_string(),
            ),
            unifi_rs::ClientOverview::Wireless(c) => (
                c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                c.base.ip_address.as_deref().unwrap_or("Unknown").to_string(),
                c.mac_address.clone(),
                "Wireless".to_string(),
            ),
            _ => (
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Other".to_string(),
            ),
        };

        Row::new(vec![name, ip, mac, r#type])
    }).collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(clients, widths)
        .header(Row::new(vec!["Name", "IP", "MAC", "Type"]))
        .block(Block::default().borders(Borders::ALL).title("Clients"))
        .row_highlight_style(Style::default().bg(Color::Gray));

    f.render_widget(table, area);
}

fn render_stats<B: Backend>(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ].as_ref())
        .split(area);

    // Summary
    let summary = vec![
        Line::from(format!("Total Sites: {}", app.sites.len())),
        Line::from(format!("Total Devices: {}", app.devices.len())),
        Line::from(format!("Total Clients: {}", app.clients.len())),
    ];

    let summary_widget = Paragraph::new(summary)
        .block(Block::default().borders(Borders::ALL).title("Summary"));
    f.render_widget(summary_widget, chunks[0]);
    
    let client_history: Vec<&NetworkStats> = app.stats_history.iter().collect();
    if !client_history.is_empty() {
        let total_data: Vec<(f64, f64)> = client_history.iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.client_count as f64))
            .collect();
        let wireless_data: Vec<(f64, f64)> = client_history.iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wireless_clients as f64))
            .collect();
        let wired_data: Vec<(f64, f64)> = client_history.iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wired_clients as f64))
            .collect();

        let datasets = vec![
            Dataset::default()
                .name("Total Clients")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Blue))
                .data(&total_data),
            Dataset::default()
                .name("Wireless")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(&wireless_data),
            Dataset::default()
                .name("Wired")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&wired_data),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().title("Client History").borders(Borders::ALL))
            .x_axis(Axis::default().bounds([0.0, 100.0]))
            .y_axis(Axis::default().bounds([0.0, client_history.iter().map(|s| s.client_count as f64).fold(0.0, f64::max)]));
        f.render_widget(chart, chunks[1]);
    }
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

