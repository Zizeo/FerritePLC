pub mod app;
pub mod canvas_fbd;
pub mod canvas_ld;
pub mod canvas_sfc;
pub mod editor_st;
pub mod editor_symbols;
pub mod i18n;

use std::path::{Path, PathBuf};

pub use app::{FerriteStudioApp, WorkspaceTab};
use ferrite_project::{MANIFEST_FILE_NAME, Project, ProjectError};
pub use i18n::UiLanguage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("Project error: {0}")]
    Project(#[from] ProjectError),

    #[error("Native GUI window error: {0}")]
    Eframe(String),
}

/// Resolves or scaffolds an initial `Project` for FerritePLC Studio.
pub fn resolve_or_scaffold_project(initial_path: Option<&Path>) -> Result<Project, ProjectError> {
    if let Some(p) = initial_path {
        if p.exists() && (p.is_file() || p.join(MANIFEST_FILE_NAME).exists()) {
            return Project::load(p);
        }
        return Project::scaffold(p, "my_automation_plant");
    }

    let cwd_manifest = PathBuf::from(MANIFEST_FILE_NAME);
    if cwd_manifest.exists() {
        return Project::load(Path::new("."));
    }

    let example_dir = PathBuf::from("examples").join("my_automation_plant");
    if example_dir.join(MANIFEST_FILE_NAME).exists() {
        return Project::load(&example_dir);
    }

    Project::scaffold(&example_dir, "my_automation_plant")
}

/// Launches the native FerritePLC Studio desktop IDE & PLC Simulator window.
pub fn run_gui(initial_path: Option<PathBuf>) -> Result<(), GuiError> {
    run_gui_with_lang(initial_path, UiLanguage::En)
}

/// Configures OS fallback fonts (`Segoe UI`, `Segoe UI Symbol`, `Consolas` on Windows)
/// so Unicode symbols and box-drawing glyphs never render as missing-glyph squares (`□`).
pub fn configure_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidate_fonts = [
        ("segoeui", r"C:\Windows\Fonts\segoeui.ttf"),
        ("seguisym", r"C:\Windows\Fonts\seguisym.ttf"),
        ("consola", r"C:\Windows\Fonts\consola.ttf"),
    ];

    let mut loaded_any = false;
    for (name, path) in candidate_fonts {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                prop.push(name.to_string());
            }
            if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                mono.push(name.to_string());
            }
            loaded_any = true;
        }
    }

    if loaded_any {
        ctx.set_fonts(fonts);
    }
}

/// Launches the native FerritePLC Studio desktop IDE & PLC Simulator window with a specific initial UI language (`En` or `Fr`).
pub fn run_gui_with_lang(initial_path: Option<PathBuf>, lang: UiLanguage) -> Result<(), GuiError> {
    let project = resolve_or_scaffold_project(initial_path.as_deref())?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("FerritePLC Studio — {}", project.manifest.name))
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "FerritePLC Studio",
        options,
        Box::new(move |cc| {
            configure_system_fonts(&cc.egui_ctx);
            Ok(Box::new(FerriteStudioApp::with_language(project, lang)))
        }),
    )
    .map_err(|e| GuiError::Eframe(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_st::highlight_st_source;
    use std::env;
    use std::fs;

    #[test]
    fn test_studio_app_initialization_scenarios_and_i18n() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_studio_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);
        assert_eq!(app.ui_lang, UiLanguage::En);
        assert!(app.diagnostics.is_empty());
        assert!(app.runtime.is_some());
        assert!(!app.detach_left_explorer);
        assert!(!app.detach_right_io);
        app.detach_left_explorer = true;
        app.detach_right_io = true;
        assert!(app.detach_left_explorer && app.detach_right_io);

        app.execute_all_scenarios();
        assert_eq!(app.scenario_logs.len(), 1);
        assert!(app.scenario_logs[0].starts_with("[PASS]"));

        // Switch UI language to French and verify localized scenario and status output
        app.ui_lang = UiLanguage::Fr;
        assert_eq!(app.ui_lang.code(), "FR");
        assert_eq!(app.ui_lang.display_name(), "Français");
        app.execute_all_scenarios();
        assert_eq!(app.scenario_logs.len(), 1);
        assert!(app.scenario_logs[0].starts_with("[SUCCÈS]"));

        let job = highlight_st_source(
            "PROGRAM test\n (* comment *)\n IF %IX0.0 AND X1.state THEN %MW10 := 42; END_IF;\nEND_PROGRAM",
        );
        assert!(!job.sections.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
