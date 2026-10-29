use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ferrite_core::{IecType, IecValue, LanguageKind};
use ferrite_project::Project;
use ferrite_runtime::PlcRuntime;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

#[derive(Parser, Debug)]
#[command(
    name = "ferrite",
    version,
    about = "FerritePLC — Deterministic IEC 61131-3 / IEC 60848 Multi-Language PLC Compiler & Simulator"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum CliUiLang {
    #[default]
    En,
    Fr,
}

impl From<CliUiLang> for ferrite_gui::UiLanguage {
    fn from(val: CliUiLang) -> Self {
        match val {
            CliUiLang::En => ferrite_gui::UiLanguage::En,
            CliUiLang::Fr => ferrite_gui::UiLanguage::Fr,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliLang {
    Ld,
    Sfc,
    St,
    Fbd,
}

impl From<CliLang> for LanguageKind {
    fn from(val: CliLang) -> Self {
        match val {
            CliLang::Ld => LanguageKind::Ld,
            CliLang::Sfc => LanguageKind::Sfc,
            CliLang::St => LanguageKind::St,
            CliLang::Fbd => LanguageKind::Fbd,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scaffolds a new FerritePLC project directory with starter LD, SFC, ST, and FBD POUs
    New {
        /// Project name (and folder name unless --path is specified)
        project_name: String,
        /// Optional target directory
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Atomically saves and formats all project manifests, symbol tables, and POU files
    Save {
        /// Project directory or FerriteProject.fplc path (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Creates a new POU in pous/ and registers it in FerriteProject.fplc
    AddPou {
        /// Name of the new POU
        name: String,
        /// IEC 61131-3 / 60848 language (ld, sfc, st, fbd)
        #[arg(short, long)]
        lang: CliLang,
        /// Project directory (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Validates syntax across all POUs, resolves cross-language symbols, and detects combinational loops
    Check {
        /// Project directory (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Runs the interactive TUI simulator or executes headless test scenarios
    Sim {
        /// Execute headless scenarios from sim/scenarios.toml without launching TUI
        #[arg(long)]
        headless: bool,
        /// Project directory (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Launches the native FerritePLC Studio graphical IDE and live PLC simulator
    Gui {
        /// Optional project directory (defaults to current directory or examples/my_automation_plant)
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Initial UI display language (en or fr)
        #[arg(short, long, default_value = "en")]
        locale: CliUiLang,
    },
    /// Exports the project to standard PLCopen TC6 XML (IEC 61131-3)
    ExportPlcopen {
        /// Destination XML file path
        #[arg(short, long)]
        output: PathBuf,
        /// Project directory (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Imports a project from standard PLCopen TC6 XML into a target folder
    ImportPlcopen {
        /// Source PLCopen XML file path
        #[arg(short, long)]
        input: PathBuf,
        /// Target folder where the imported project will be created
        #[arg(short, long)]
        target_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            ferrite_gui::run_gui(None)?;
        }
        Some(Commands::Gui { path, locale }) => {
            ferrite_gui::run_gui_with_lang(path, locale.into())?;
        }
        Some(Commands::New { project_name, path }) => {
            let target_dir = path.unwrap_or_else(|| PathBuf::from(&project_name));
            let project = Project::scaffold(&target_dir, &project_name)
                .with_context(|| format!("Failed to scaffold project '{project_name}'"))?;
            println!(
                "Created FerritePLC project '{}' at {}",
                project.manifest.name,
                target_dir.display()
            );
            println!("  - Manifest: {}/FerriteProject.fplc", target_dir.display());
            println!(
                "  - POUs:     main_safety (LD), batch_seq (SFC), cip_clean (SFC), math_pid (ST), pump_ctrl (FBD)"
            );
        }
        Some(Commands::Save { path }) => {
            let project = Project::load(&path)
                .with_context(|| format!("Failed to load project from {}", path.display()))?;
            let root = project.root_dir.clone();
            project.save(&root).with_context(|| {
                format!("Failed to atomically save project at {}", root.display())
            })?;
            println!(
                "Atomically saved project '{}' at {}",
                project.manifest.name,
                root.display()
            );
        }
        Some(Commands::AddPou { name, lang, path }) => {
            let mut project = Project::load(&path)
                .with_context(|| format!("Failed to load project from {}", path.display()))?;
            let kind: LanguageKind = lang.into();
            let created = project
                .add_pou(&name, kind)
                .with_context(|| format!("Failed to add POU '{name}' ({kind})"))?;
            println!("Added {kind} POU '{name}' -> {}", created.display());
        }
        Some(Commands::Check { path }) => {
            run_check(&path)?;
        }
        Some(Commands::Sim { headless, path }) => {
            if headless {
                run_headless_sim(&path)?;
            } else {
                run_tui_sim(&path)?;
            }
        }
        Some(Commands::ExportPlcopen { output, path }) => {
            let project = Project::load(&path)
                .with_context(|| format!("Failed to load project from {}", path.display()))?;
            let xml = project
                .export_plcopen_xml()
                .with_context(|| "Failed to export PLCopen TC6 XML")?;
            std::fs::write(&output, xml)
                .with_context(|| format!("Failed to write XML to {}", output.display()))?;
            println!(
                "Successfully exported PLCopen TC6 XML for '{}' -> {}",
                project.manifest.name,
                output.display()
            );
        }
        Some(Commands::ImportPlcopen { input, target_dir }) => {
            let xml = std::fs::read_to_string(&input)
                .with_context(|| format!("Failed to read XML file {}", input.display()))?;
            let project = Project::import_plcopen_xml(&xml, &target_dir).with_context(|| {
                format!("Failed to import PLCopen XML into {}", target_dir.display())
            })?;
            println!(
                "Successfully imported PLCopen project '{}' into {}",
                project.manifest.name,
                target_dir.display()
            );
        }
    }

    Ok(())
}

fn run_check(path: &Path) -> Result<()> {
    let project = Project::load(path)
        .with_context(|| format!("Failed to load project from {}", path.display()))?;
    let diagnostics = project.check()?;

    if diagnostics.is_empty() {
        let (tags, pous) = project.compile()?;
        println!(
            "CHECK OK: Project '{}' verified successfully ({} POUs, {} tags, {} FBs).",
            project.manifest.name,
            pous.len(),
            tags.tags().len(),
            tags.fbs().len()
        );
        Ok(())
    } else {
        for diag in &diagnostics {
            eprintln!(
                "[{:?}] POU '{}' ({}): {}",
                diag.severity, diag.pou, diag.language, diag.message
            );
        }
        bail!(
            "Project '{}' failed validation with {} error(s).",
            project.manifest.name,
            diagnostics.len()
        );
    }
}

fn run_headless_sim(path: &Path) -> Result<()> {
    let project = Project::load(path)
        .with_context(|| format!("Failed to load project from {}", path.display()))?;
    let diagnostics = project.check()?;
    if !diagnostics.is_empty() {
        for diag in &diagnostics {
            eprintln!(
                "[{:?}] POU '{}' ({}): {}",
                diag.severity, diag.pou, diag.language, diag.message
            );
        }
        bail!("Cannot start simulation: project has validation errors.");
    }

    if project.scenarios.scenarios.is_empty() {
        let mut runtime = PlcRuntime::from_project(&project)?;
        let scans = 10u64;
        for _ in 0..scans {
            runtime.step_scan()?;
        }
        println!(
            "Executed {scans} headless scan cycles (virtual time = {} ms, max scan = {} us).",
            runtime.virtual_time_ms, runtime.max_scan_duration_us
        );
        return Ok(());
    }

    println!(
        "Running {} headless scenario(s) for project '{}'...",
        project.scenarios.scenarios.len(),
        project.manifest.name
    );
    for scenario in &project.scenarios.scenarios {
        let mut runtime = PlcRuntime::from_project(&project)?;
        let report = runtime.run_scenario(scenario)?;
        println!(
            "  [PASS] Scenario '{}': {} scans, {} assertions verified (max scan: {} us)",
            report.scenario_name,
            report.scans_executed,
            report.assertions_verified,
            report.max_scan_duration_us
        );
    }
    Ok(())
}

fn run_tui_sim(path: &Path) -> Result<()> {
    let project = Project::load(path)
        .with_context(|| format!("Failed to load project from {}", path.display()))?;
    let diagnostics = project.check()?;
    if !diagnostics.is_empty() {
        bail!(
            "Project has {} validation error(s); run `ferrite check` for details.",
            diagnostics.len()
        );
    }

    let mut runtime = PlcRuntime::from_project(&project)?;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = tui_loop(&mut terminal, &project, &mut runtime);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: &Project,
    runtime: &mut PlcRuntime,
) -> Result<()> {
    let mut running = false;
    let mut selected_idx: usize = 0;
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let tick_rate = Duration::from_millis(runtime.scan_cycle_ms.max(20));
    let mut last_tick = Instant::now();

    loop {
        let tag_count = runtime.tags.tags().len();
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(6),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let status_str = if running { "RUNNING" } else { "PAUSED" };
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" FerritePLC — {} ", project.manifest.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "| State: {status_str} | Scan: #{} | Virtual Clock: {} ms | Scan Time: {} us (Max: {} us)",
                    runtime.scan_count,
                    runtime.virtual_time_ms,
                    runtime.last_scan_duration_us,
                    runtime.max_scan_duration_us
                )),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Telemetry & Watchdog"));
            f.render_widget(header, chunks[0]);

            let rows: Vec<Row> = runtime
                .tags
                .tags()
                .iter()
                .map(|t| {
                    let addr_str = t
                        .address
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let val_str = runtime
                        .read_value(&t.name)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|_| "ERR".to_string());
                    let mode_str = if t.forced.is_some() {
                        "FORCED"
                    } else if t.read_only_outside_sfc {
                        "SFC-RO"
                    } else {
                        "RW"
                    };
                    Row::new(vec![
                        Cell::from(t.name.clone()),
                        Cell::from(t.data_type.to_string()),
                        Cell::from(addr_str),
                        Cell::from(val_str),
                        Cell::from(mode_str),
                        Cell::from(t.comment.clone()),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(22),
                    Constraint::Percentage(8),
                    Constraint::Percentage(10),
                    Constraint::Percentage(14),
                    Constraint::Percentage(10),
                    Constraint::Percentage(36),
                ],
            )
            .header(
                Row::new(vec!["Symbol", "Type", "Address", "Value", "Mode", "Comment"]).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Unified Symbol Table & Shared Process Image"),
            )
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_stateful_widget(table, chunks[1], &mut table_state);

            let fb_lines: Vec<Line> = runtime
                .tags
                .fbs()
                .iter()
                .map(|fb| {
                    Line::from(format!(
                        "  {} ({:?}): IN={} | Q={} | ET={}ms / PT={}ms | CV={}",
                        fb.name, fb.kind, fb.in_bool, fb.q, fb.et_ms, fb.pt_ms, fb.cv
                    ))
                })
                .collect();
            let fb_panel = Paragraph::new(fb_lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Shared Function Block Instances"),
            );
            f.render_widget(fb_panel, chunks[2]);

            let footer = Paragraph::new(
                " Controls: [Space] Run/Pause | [s] Single Step Scan | [Up/Down] Select Symbol | [Enter/t] Toggle BOOL | [q] Quit",
            )
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[3]);
        })?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char(' ') => running = !running,
                KeyCode::Char('s') => {
                    let _ = runtime.step_scan();
                }
                KeyCode::Down => {
                    if tag_count > 0 {
                        selected_idx = (selected_idx + 1) % tag_count;
                        table_state.select(Some(selected_idx));
                    }
                }
                KeyCode::Up => {
                    if tag_count > 0 {
                        selected_idx = if selected_idx == 0 {
                            tag_count - 1
                        } else {
                            selected_idx - 1
                        };
                        table_state.select(Some(selected_idx));
                    }
                }
                KeyCode::Enter | KeyCode::Char('t') => {
                    if let Some(tag) = runtime.tags.tags().get(selected_idx).cloned()
                        && tag.data_type == IecType::Bool
                        && !tag.read_only_outside_sfc
                    {
                        let cur = runtime
                            .read_value(&tag.name)
                            .and_then(|v| Ok(v.as_bool()?))
                            .unwrap_or(false);
                        let _ = runtime.set_stimulus(&tag.name, IecValue::Bool(!cur));
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            if running {
                let _ = runtime.step_scan();
            }
            last_tick = Instant::now();
        }
    }

    Ok(())
}
