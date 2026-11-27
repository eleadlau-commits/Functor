#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use functor_paper::glyph;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[derive(Default)]
struct Files {
    watchers: Mutex<HashMap<PathBuf, RecommendedWatcher>>,
    last_written: Mutex<HashMap<PathBuf, String>>,
}

#[derive(Serialize, Clone)]
struct Doc {
    path: String,
    name: String,
    text: String,
}

fn doc(path: &Path, text: String) -> Doc {
    Doc {
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        text,
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

fn last_file_store(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("last-file.txt"))
}

fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn watch(app: &AppHandle, files: &Files, path: &Path) -> Result<(), String> {
    let target = key(path);
    if files.watchers.lock().unwrap().contains_key(&target) {
        return Ok(());
    }
    let dir = target.parent().ok_or("The file has no folder.")?.to_path_buf();
    let handle = app.clone();
    let wanted = target.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
            return;
        }
        if !event.paths.iter().any(|p| p.canonicalize().map(|c| c == wanted).unwrap_or(false)) {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&wanted) else { return };
        let files = handle.state::<Files>();
        let echo = files.last_written.lock().unwrap().get(&wanted).map(|t| t == &text).unwrap_or(false);
        if !echo {
            let _ = handle.emit("file-changed", doc(&wanted, text));
        }
    })
    .map_err(|e| e.to_string())?;
    watcher.watch(&dir, RecursiveMode::NonRecursive).map_err(|e| e.to_string())?;
    files.watchers.lock().unwrap().insert(target, watcher);
    Ok(())
}

fn load(app: &AppHandle, files: &Files, path: &Path) -> Result<Doc, String> {
    let text = read(path)?;
    let path = key(path);
    watch(app, files, &path)?;
    Ok(doc(&path, text))
}

fn glyph_store(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_cache_dir().ok().map(|d| d.join("glyphs.txt"))
}

thread_local! {
    static CACHE_LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn with_glyph_cache<T>(app: &AppHandle, f: impl FnOnce() -> T) -> T {
    let file = glyph_store(app);
    if !CACHE_LOADED.with(|c| c.replace(true)) {
        if let Some(text) = file.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            glyph::import_cache(&text);
        }
    }
    let out = f();
    if let Some(file) = file.filter(|_| glyph::take_dirty()) {
        let tmp = file.with_extension("tmp");
        let _ = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&tmp, glyph::export_cache()))
            .and_then(|_| std::fs::rename(&tmp, &file));
    }
    out
}

#[tauri::command]
fn compile(app: AppHandle, src: String, width: Option<f64>) -> String {
    let t = Instant::now();
    let out = with_glyph_cache(&app, || functor_paper::compile_width(&src, width));
    let json = out.to_json();
    format!("{{\"nativeMs\":{},{}", t.elapsed().as_secs_f64() * 1000.0, &json[1..])
}

thread_local! {
    static SESSION: std::cell::RefCell<functor_paper::incr::Session> = std::cell::RefCell::new(functor_paper::incr::Session::new());
}

#[tauri::command]
fn compile_parts(app: AppHandle, src: String, width: Option<f64>, reset: bool) -> String {
    let t = Instant::now();
    let json = SESSION.with(|s| {
        let mut s = s.borrow_mut();
        if reset {
            s.reset();
        }
        with_glyph_cache(&app, || s.compile(&src, width)).to_json().to_string()
    });
    format!("{{\"nativeMs\":{},{}", t.elapsed().as_secs_f64() * 1000.0, &json[1..])
}

#[tauri::command]
fn glyph(app: AppHandle, recipe: String) -> String {
    with_glyph_cache(&app, || functor_paper::glyph_svg(&recipe)).unwrap_or_default()
}

#[tauri::command]
fn symbols() -> String {
    functor_paper::symbols_json()
}

#[tauri::command]
fn initial_file(app: AppHandle, files: State<Files>, last: bool) -> Option<Doc> {
    let from_args = std::env::args().skip(1).find(|a| !a.starts_with('-')).map(PathBuf::from);
    let from_store = last_file_store(&app).filter(|_| last).and_then(|s| std::fs::read_to_string(s).ok()).map(|s| PathBuf::from(s.trim()));
    from_args.into_iter().chain(from_store).find(|p| p.is_file()).and_then(|p| load(&app, &files, &p).ok())
}

#[tauri::command]
fn load_file(app: AppHandle, files: State<Files>, path: String) -> Option<Doc> {
    let path = PathBuf::from(path);
    path.is_file().then(|| load(&app, &files, &path).ok()).flatten()
}

#[tauri::command]
async fn open_file(app: AppHandle, files: State<'_, Files>) -> Result<Vec<Doc>, String> {
    let picked = app.dialog().file().add_filter("Functor", &["fn"]).blocking_pick_files().unwrap_or_default();
    picked.into_iter().filter_map(|p| p.into_path().ok()).map(|path| load(&app, &files, &path)).collect()
}

#[tauri::command]
fn save_file(app: AppHandle, files: State<Files>, path: String, text: String) -> Result<Doc, String> {
    let path = PathBuf::from(path);
    files.last_written.lock().unwrap().insert(key(&path), text.clone());
    std::fs::write(&path, &text).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    let path = key(&path);
    files.last_written.lock().unwrap().insert(path.clone(), text.clone());
    watch(&app, &files, &path)?;
    Ok(doc(&path, text))
}

#[tauri::command]
async fn save_as(app: AppHandle, files: State<'_, Files>, name: String, text: String) -> Result<Option<Doc>, String> {
    let picked = app.dialog().file().add_filter("Functor", &["fn"]).set_file_name(&name).blocking_save_file();
    let Some(mut path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    if path.extension().is_none() {
        path.set_extension("fn");
    }
    files.last_written.lock().unwrap().insert(key(&path), text.clone());
    std::fs::write(&path, &text).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    files.last_written.lock().unwrap().insert(key(&path), text.clone());
    load(&app, &files, &path).map(Some)
}

#[tauri::command]
async fn export_html(app: AppHandle, name: String, html: String) -> Result<Option<String>, String> {
    let picked = app.dialog().file().add_filter("HTML", &["html"]).set_file_name(&name).blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    std::fs::write(&path, html).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn export_pdf(app: AppHandle, name: String, src: String) -> Result<Option<String>, String> {
    let picked = app.dialog().file().add_filter("PDF", &["pdf"]).set_file_name(&name).blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let out = functor_paper::compile_pdf(&src);
    std::fs::write(&path, out.pdf).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
fn close_file(app: AppHandle, files: State<Files>, path: String) {
    let path = key(Path::new(&path));
    files.watchers.lock().unwrap().remove(&path);
    files.last_written.lock().unwrap().remove(&path);
    if let Some(store) = last_file_store(&app) {
        let _ = std::fs::remove_file(store);
    }
}

#[tauri::command]
async fn ask_discard(app: AppHandle, name: String) -> bool {
    app.dialog()
        .message(format!("{name} has changes that are not saved. Close it and lose the changes?"))
        .title("Close tab")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Close without saving".into(), "Cancel".into()))
        .blocking_show()
}

#[cfg(target_os = "macos")]
fn mac_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    let item = |id: &str, text: &str, keys: &str| MenuItemBuilder::with_id(id, text).accelerator(keys).build(app);
    let main = SubmenuBuilder::new(app, "Functor").about(None).separator().services().separator().hide().hide_others().show_all().separator().quit().build()?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new-tab", "New Tab", "CmdOrCtrl+T")?)
        .item(&item("open", "Open…", "CmdOrCtrl+O")?)
        .separator()
        .item(&item("close-tab", "Close Tab", "CmdOrCtrl+W")?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .item(&item("next-tab", "Show Next Tab", "Ctrl+Tab")?)
        .item(&item("prev-tab", "Show Previous Tab", "Ctrl+Shift+Tab")?)
        .separator()
        .minimize()
        .fullscreen()
        .build()?;
    MenuBuilder::new(app).item(&main).item(&file).item(&edit).item(&window).build()
}

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.menu(mac_menu);
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Files::default())
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                window.app_handle().exit(0);
            }
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.clone());
        })
        .invoke_handler(tauri::generate_handler![
            compile,
            compile_parts,
            glyph,
            symbols,
            initial_file,
            load_file,
            ask_discard,
            open_file,
            save_file,
            save_as,
            export_html,
            export_pdf,
            close_file
        ])
        .run(tauri::generate_context!())
        .expect("Functor did not start");
}
