use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use sva_core::types::StableSliceGraphJson;
use sva_core::wave::{WaveformReader, WellenReader};
use sva_tui::app::ExplorerState;
use sva_tui::graph::GraphIndex;
use sva_tui::ui::{self, WaveDisplay};

const DEFAULT_JSON: &str = "ibex_blues_slice.json";
const DEFAULT_ROOT_BLOCK_ID: u64 = 1480;
const DEFAULT_ROOT_TIME: i64 = 19;

#[derive(Debug, Parser)]
#[command(name = "sva_tui")]
#[command(about = "Interactively explore a Blues slice graph as an upstream dependency tree")]
struct Cli {
    #[arg(default_value = DEFAULT_JSON)]
    json_path: PathBuf,
    #[arg(long, default_value_t = DEFAULT_ROOT_BLOCK_ID)]
    root_block_id: u64,
    #[arg(long, default_value_t = DEFAULT_ROOT_TIME)]
    root_time: i64,
    #[arg(long)]
    full_signal: bool,
    #[arg(long)]
    vcd: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let graph = load_graph(&cli.json_path)?;
    let index = GraphIndex::new(graph);
    let root_id = index.find_root_node(cli.root_block_id, cli.root_time)?;
    let root_signal = Some(index.target().to_string());
    let mut state = ExplorerState::new(index, root_id, root_signal);
    let waveform = cli
        .vcd
        .as_ref()
        .map(WellenReader::open)
        .transpose()
        .with_context(|| "failed to open --vcd waveform")?;

    run_terminal(&mut state, cli.full_signal, waveform.as_ref())
}

fn load_graph(path: &PathBuf) -> Result<StableSliceGraphJson> {
    let file = File::open(path)
        .with_context(|| format!("failed to open slice JSON {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse slice JSON {}", path.display()))
}

fn run_terminal(
    state: &mut ExplorerState,
    full_signal: bool,
    waveform: Option<&WellenReader>,
) -> Result<()> {
    enable_raw_mode().context("failed to enable raw terminal mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
    let result = run_app(&mut terminal, state, full_signal, waveform);

    disable_raw_mode().context("failed to disable raw terminal mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut ExplorerState,
    full_signal: bool,
    waveform: Option<&WellenReader>,
) -> Result<()> {
    loop {
        let wave = current_wave_display(state, waveform);
        terminal
            .draw(|frame| ui::draw(frame, state, full_signal, wave.as_ref()))
            .context("failed to draw terminal UI")?;

        if !event::poll(Duration::from_millis(250)).context("failed to poll terminal events")? {
            continue;
        }

        let Event::Key(key) = event::read().context("failed to read terminal event")? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => state.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => state.move_selection(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                state.expand_selected()
            }
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') | KeyCode::Char('H') => {
                state.collapse_or_parent()
            }
            KeyCode::Char('r') | KeyCode::Char('R') => state.reset(),
            KeyCode::PageDown | KeyCode::Char(']') => {
                state.scroll_code(10, 1);
            }
            KeyCode::PageUp | KeyCode::Char('[') => {
                state.scroll_code(-10, 1);
            }
            _ => {}
        }
    }
}

fn current_wave_display(
    state: &mut ExplorerState,
    waveform: Option<&WellenReader>,
) -> Option<WaveDisplay> {
    let waveform = waveform?;
    let Some(query) = state.selected_wave_query() else {
        return Some(WaveDisplay::NoSignal);
    };
    let signal = query.signal.name.clone();
    let time = query.time.0;

    Some(match waveform.signal_value_at(&query.signal, query.time) {
        Ok(Some(value)) => WaveDisplay::Value {
            signal,
            time,
            raw_bits: value.raw_bits,
            pretty_hex: value.pretty_hex,
        },
        Ok(None) => WaveDisplay::Missing { signal, time },
        Err(error) => WaveDisplay::Error {
            signal,
            time,
            message: error.to_string(),
        },
    })
}
