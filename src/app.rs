use crate::wifi::WifiStats;
use crate::config::Config;
use crate::network::ProbeAlert;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use std::collections::{HashSet, VecDeque};
use chrono::Local;

pub struct App {
    pub wifi_stats: Option<WifiStats>,
    pub avg_signal: f32,
    pub peers: HashSet<String>,
    pub alerts: VecDeque<ProbeAlert>,
    pub logs: VecDeque<String>,
    pub local_peer_id: String,
    pub config: Config,
    pub should_quit: bool,
}

impl App {
    pub fn new(local_peer_id: String, config: Config) -> Self {
        Self {
            wifi_stats: None,
            avg_signal: 0.0,
            peers: HashSet::new(),
            alerts: VecDeque::with_capacity(50),
            logs: VecDeque::with_capacity(100),
            local_peer_id,
            config,
            should_quit: false,
        }
    }

    pub fn add_log(&mut self, log: String) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.logs.push_back(format!("[{}] {}", timestamp, log));
        if self.logs.len() > 100 {
            self.logs.pop_front();
        }
    }

    pub fn add_alert(&mut self, alert: ProbeAlert) {
        self.alerts.push_back(alert);
        if self.alerts.len() > 50 {
            self.alerts.pop_front();
        }
    }

    pub fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(7), // Stats
                Constraint::Min(10),   // Middle (Peers & Alerts)
                Constraint::Length(10), // Logs
            ])
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_stats(f, chunks[1]);
        self.render_middle(f, chunks[2]);
        self.render_logs(f, chunks[3]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let header = Paragraph::new(Line::from(vec![
            Span::styled(" NetProbe TUI ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Span::raw(" | Local Peer ID: "),
            Span::styled(&self.local_peer_id, Style::default().fg(Color::Yellow)),
            Span::raw(" | Press 'q' to quit"),
        ]))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, area);
    }

    fn render_stats(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Gauge
                Constraint::Percentage(60), // Text stats
            ])
            .split(area);

        let signal = self.wifi_stats.as_ref().map(|s| s.signal).unwrap_or(0);
        let gauge_color = if signal > 70 { Color::Green } else if signal > 40 { Color::Yellow } else { Color::Red };
        
        let gauge = Gauge::default()
            .block(Block::default().title(" Signal Strength ").borders(Borders::ALL))
            .gauge_style(Style::default().fg(gauge_color))
            .percent(signal as u16)
            .label(format!("{}% (Avg: {:.1}%)", signal, self.avg_signal));
        f.render_widget(gauge, chunks[0]);

        let stats_text = if let Some(stats) = &self.wifi_stats {
            vec![
                Line::from(vec![Span::raw("BSSID: "), Span::styled(&stats.bssid, Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("Channel: "), Span::styled(stats.channel.to_string(), Style::default().fg(Color::White))]),
                Line::from(vec![
                    Span::raw("Rates: RX "), 
                    Span::styled(format!("{:.1} Mbps", stats.receive_rate), Style::default().fg(Color::Green)),
                    Span::raw(" / TX "),
                    Span::styled(format!("{:.1} Mbps", stats.transmit_rate), Style::default().fg(Color::Green)),
                ]),
            ]
        } else {
            vec![Line::from("Waiting for data...")]
        };

        let stats_par = Paragraph::new(stats_text)
            .block(Block::default().title(" Interface Info ").borders(Borders::ALL));
        f.render_widget(stats_par, chunks[1]);
    }

    fn render_middle(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Peers
                Constraint::Percentage(70), // Alerts
            ])
            .split(area);

        let peers: Vec<ListItem> = self.peers.iter()
            .map(|p| ListItem::new(Span::styled(p, Style::default().fg(Color::Blue))))
            .collect();
        let peers_list = List::new(peers)
            .block(Block::default().title(format!(" Peers ({}) ", self.peers.len())).borders(Borders::ALL));
        f.render_widget(peers_list, chunks[0]);

        let alerts: Vec<ListItem> = self.alerts.iter().rev()
            .map(|a| {
                let time = chrono::DateTime::from_timestamp(a.timestamp, 0)
                    .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                
                let content = vec![
                    Line::from(vec![
                        Span::styled(format!("[{}] ALERT from ", time), Style::default().fg(Color::Red)),
                        Span::styled(if a.peer_id.len() > 8 { &a.peer_id[..8] } else { &a.peer_id }, Style::default().fg(Color::Yellow)),
                        Span::raw(format!(": Sig {}% (Avg {:.1}%)", a.signal, a.avg_signal)),
                    ]),
                    Line::from(vec![
                        Span::raw(format!("      BSSID: {}, Ch: {}", a.bssid, a.channel)),
                    ]),
                ];
                ListItem::new(content)
            })
            .collect();
        let alerts_list = List::new(alerts)
            .block(Block::default().title(" Alert History ").borders(Borders::ALL));
        f.render_widget(alerts_list, chunks[1]);
    }

    fn render_logs(&self, f: &mut Frame, area: Rect) {
        let logs: Vec<ListItem> = self.logs.iter().rev()
            .map(|l| ListItem::new(l.as_str()))
            .collect();
        let logs_list = List::new(logs)
            .block(Block::default().title(" Logs ").borders(Borders::ALL));
        f.render_widget(logs_list, area);
    }
}
