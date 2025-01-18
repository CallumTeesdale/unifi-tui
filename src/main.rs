use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::{
    io,
    time::{Duration, Instant},
};
use unifi_rs::{UnifiClient, UnifiClientBuilder};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// UniFi Controller URL
    #[arg(long)]
    #[clap(env = "UNIFI_URL")]
    url: String,

    /// API Key
    #[arg(long)]
    #[clap(env = "UNIFI_API_KEY")]
    api_key: String,

    /// Skip SSL verification
    #[arg(long, default_value = "false")]
    insecure: bool,
}

#[derive(Clone)]
struct App {
    client: UnifiClient,
    sites: Vec<unifi_rs::SiteOverview>,
    devices: Vec<unifi_rs::DeviceOverview>,
    clients: Vec<unifi_rs::ClientOverview>,
    current_tab: usize,
    should_quit: bool,
    refresh_interval: Duration,
    last_update: Instant,
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
            should_quit: false,
            refresh_interval: Duration::from_secs(5),
            last_update: Instant::now(),
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
    let menu_titles = ["Sites", "Devices", "Clients", "Stats"];

    loop {
        if let Err(e) = app.refresh_data().await {
            eprintln!("Error refreshing data: {}", e);
        }

        terminal.draw(|f| {
            let size = f.area();

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
                .split(size);

            let menu = menu_titles
                .iter()
                .map(|t| Line::from(*t))
                .collect::<Vec<Line>>();
            let tabs = Tabs::new(menu)
                .block(Block::default().borders(Borders::ALL).title("Tabs"))
                .select(app.current_tab)
                .style(Style::default())
                .highlight_style(Style::default().bold());
            f.render_widget(tabs, chunks[0]);

            match app.current_tab {
                0 => render_sites(f, chunks[1], &app),
                1 => render_devices(f, chunks[1], &app),
                2 => render_clients(f, chunks[1], &app),
                3 => render_stats(f, chunks[1], &app),
                _ => {}
            }

            let help_text = vec![
                Line::from("Press 'q' to quit"),
                Line::from("Press 'tab' to switch views"),
                Line::from("Press 'r' to refresh data"),
            ];
            let help = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Help"));
            f.render_widget(help, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                        break;
                    }
                    KeyCode::Tab => {
                        app.current_tab = (app.current_tab + 1) % 4;
                    }
                    KeyCode::Char('r') => {
                        app.last_update = Instant::now() - app.refresh_interval;
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
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

    let widths = &[Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = Table::new(sites, widths)
        .header(Row::new(vec!["ID", "Name"]))
        .block(Block::default().borders(Borders::ALL).title("Sites"));

    f.render_widget(table, area);
}

fn render_devices(f: &mut Frame, area: Rect, app: &App) {
    let devices: Vec<Row> = app
        .devices
        .iter()
        .map(|device| {
            let cells = vec![
                Cell::from(device.name.clone()),
                Cell::from(device.model.clone()),
                Cell::from(device.mac_address.clone()),
                Cell::from(device.ip_address.clone()),
                Cell::from(format!("{:?}", device.state)),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = &[
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];

    let table = Table::new(devices, widths)
        .header(Row::new(vec!["Name", "Model", "MAC", "IP", "State"]))
        .block(Block::default().borders(Borders::ALL).title("Devices"));

    f.render_widget(table, area);
}

fn render_clients(f: &mut Frame, area: Rect, app: &App) {
    let clients: Vec<Row> = app
        .clients
        .iter()
        .map(|client| {
            let (name, ip, mac, r#type) = match client {
                unifi_rs::ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    c.mac_address.clone(),
                    "Wired".to_string(),
                ),
                unifi_rs::ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
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
        })
        .collect();

    let widths = &[
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(clients, widths)
        .header(Row::new(vec!["Name", "IP", "MAC", "Type"]))
        .block(Block::default().borders(Borders::ALL).title("Clients"));

    f.render_widget(table, area);
}

fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    let stats = vec![
        Line::from(format!("Total Sites: {}", app.sites.len())),
        Line::from(format!("Total Devices: {}", app.devices.len())),
        Line::from(format!("Total Clients: {}", app.clients.len())),
        Line::from(""),
        Line::from(format!("Last Update: {:?} ago", app.last_update.elapsed())),
        Line::from(format!("Refresh Interval: {:?}", app.refresh_interval)),
    ];

    let paragraph = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title("Statistics"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}
