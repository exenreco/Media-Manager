#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ads;
mod backend;
mod branding;
mod library_tools;
mod media_player;
#[cfg(any(target_os = "android", target_os = "ios", test))]
mod mobile_storage;
mod notifications;
mod pairing;
mod poster;
mod preview;
mod profiles;
mod session;
mod workspace;

use backend::Connection;
use dioxus::prelude::*;
use tv_manager_core::{
    rename::{episode_name, RenamePlan, TaskStatus},
    PlanRequest, Source,
};

const CSS: &str = concat!(
    include_str!("style.css"),
    "\n",
    include_str!("workspace.css")
);

#[derive(Clone, Copy)]
struct PanelDrag {
    right: bool,
    x: f64,
    width: f64,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Form {
    name: String,
    year: String,
    season: String,
    movie: bool,
    plex: bool,
    jellyfin: bool,
    folder: String,
    source: String,
    key: String,
    custom: String,
    titles: String,
    guide: String,
    template: String,
    download_posters: bool,
    options: tv_manager_core::LibraryOptions,
}
impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            year: String::new(),
            season: String::new(),
            movie: false,
            plex: false,
            jellyfin: false,
            folder: String::new(),
            source: "tvmaze".into(),
            key: String::new(),
            custom: String::new(),
            titles: String::new(),
            guide: String::new(),
            template: "{Series Name} - S{Season:02d}E{Episode:02d}".into(),
            download_posters: false,
            options: tv_manager_core::LibraryOptions {
                companions: true,
                absolute_start: 1,
                ..Default::default()
            },
        }
    }
}
impl Form {
    fn request(&self) -> anyhow::Result<PlanRequest> {
        let season = if self.movie {
            None
        } else if self.season.trim().is_empty() {
            // Untagged manual titles and sequences describe one season.
            matches!(self.source.as_str(), "manual" | "batch").then_some(1)
        } else {
            Some(
                self.season
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| anyhow::anyhow!("Enter a valid season number"))?,
            )
        };
        let year = if self.year.trim().is_empty() {
            None
        } else {
            Some(
                self.year
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| anyhow::anyhow!("Enter a valid year"))?,
            )
        };
        let source = match self.source.as_str() {
            "omdb" => Source::Omdb {
                key: self.key.clone(),
            },
            "custom" => Source::Custom {
                base_url: self.custom.clone(),
                key: self.key.clone(),
            },
            "manual" => Source::Manual {
                titles: self.titles.clone(),
            },
            "batch" => Source::Batch {
                template: self.template.clone(),
            },
            "web" => Source::Web {
                url: self.guide.clone(),
            },
            _ => Source::TvMaze,
        };
        Ok(PlanRequest {
            folder: self.folder.clone(),
            name: self.name.clone(),
            year,
            season,
            movie: self.movie,
            plex: self.plex,
            jellyfin: self.jellyfin,
            source,
            download_posters: self.download_posters,
            options: self.options.clone(),
        })
    }
}

#[component]
fn MobileStorageBridge() -> Element {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return rsx! { mobile_storage::Bridge {} };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    rsx! {}
}

fn main() {
    #[cfg(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    dioxus::LaunchBuilder::desktop()
        .with_cfg(branding::desktop_config())
        .launch(App);
    #[cfg(not(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    )))]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut resume = use_signal(|| None::<session::Session>);
    let mut persistence_ready = use_signal(|| false);
    let mut form = use_signal(Form::default);
    let mut connection = use_signal(Connection::default);
    let mut plan = use_signal(|| None::<RenamePlan>);
    let mut snapshot = use_signal(|| None::<(Form, Connection)>);
    let mut busy = use_signal(|| false);
    let mut message = use_signal(String::new);
    let mut persistence_error = use_signal(String::new);
    let mut confirming = use_signal(|| false);
    let mut consumed = use_signal(|| false);
    let mut show_poster_help = use_signal(|| false);
    let mut selected = use_signal(|| None::<usize>);
    let mut search = use_signal(String::new);
    let mut left_width = use_signal(|| 340.0f64);
    let mut right_width = use_signal(|| 300.0f64);
    let mut active_panel = use_signal(|| "preview");
    let mut drag = use_signal(|| None::<PanelDrag>);
    let mut splash_visible = use_signal(|| {
        cfg!(all(
            feature = "desktop",
            not(any(target_os = "android", target_os = "ios"))
        ))
    });
    let mut startup_progress = use_signal(|| 10u8);
    let mut splash_artwork_ready = use_signal(|| false);
    use_future(move || async move {
        // Yield to the first render so the splash can paint before disk/decryption work.
        tokio::task::yield_now().await;
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if let Err(error) = mobile_storage::initialize_settings().await {
            message.set(format!("Could not open app settings: {error:#}"));
            persistence_ready.set(true);
            return;
        }
        let saved = tokio::task::spawn_blocking(session::load).await;
        match saved {
            Ok(Ok(previous)) => {
                persistence_ready.set(previous.is_none());
                resume.set(previous);
            }
            _ => {
                message.set(
                    "The previous workspace could not be read. You can start a new workspace."
                        .into(),
                );
                persistence_ready.set(true);
            }
        }
        startup_progress.set(70);
        if splash_visible() {
            while !splash_artwork_ready() {
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
            }
        }
        startup_progress.set(100);
        // Let the completed bar paint before switching to the dashboard.
        if splash_visible() {
            tokio::time::sleep(std::time::Duration::from_millis(180)).await;
            if splash_visible() {
                branding::finish_splash();
                splash_visible.set(false);
            }
        }
    });

    // Keep a nonreactive latest snapshot for shutdown and debounce disk writes.
    let latest_session =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<session::Session>)));
    let pending_save =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<dioxus::core::Task>)));
    let autosave_latest = latest_session.clone();
    use_effect(move || {
        let state = session::Session::new(
            form(),
            connection(),
            left_width(),
            right_width(),
            plan.read().is_some(),
        );
        if !persistence_ready() {
            return;
        }
        if !state.has_work() && autosave_latest.borrow().is_none() {
            return;
        }
        *autosave_latest.borrow_mut() = Some(state.clone());
        if let Some(task) = pending_save.borrow_mut().take() {
            task.cancel();
        }
        *pending_save.borrow_mut() = Some(spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            match session::save(&state) {
                Ok(()) => persistence_error.set(String::new()),
                Err(_) => persistence_error.set(
                    "Workspace settings could not be saved. Check access to the app data folder."
                        .into(),
                ),
            }
        }));
    });
    use_drop(move || {
        if let Some(state) = latest_session.borrow().as_ref() {
            let _ = session::save(state);
        }
    });

    let changed = snapshot
        .read()
        .as_ref()
        .is_some_and(|(f, c)| *f != *form.read() || *c != *connection.read());
    let rows = plan
        .read()
        .as_ref()
        .map(|p| p.tasks.clone())
        .unwrap_or_default();
    let missing = plan
        .read()
        .as_ref()
        .map(|p| p.missing.clone())
        .unwrap_or_default();
    let ready = rows
        .iter()
        .filter(|t| t.status == TaskStatus::Ready)
        .count();
    let conflict = rows.iter().any(|t| t.status == TaskStatus::Conflict);
    let posters = plan
        .read()
        .as_ref()
        .map(|p| p.posters.clone())
        .unwrap_or_default();
    let poster_count = posters
        .iter()
        .filter(|p| p.status == TaskStatus::Ready)
        .count();
    let can_apply =
        (ready > 0 || poster_count > 0) && !conflict && !changed && !busy() && !consumed();
    let form_value = form.read().clone();
    let connection_value = connection.read().clone();
    let preview_request = snapshot
        .read()
        .as_ref()
        .and_then(|(f, _)| f.request().ok())
        .unwrap_or_default();
    let preview_show = plan
        .read()
        .as_ref()
        .map(|p| p.show.clone())
        .unwrap_or_default();
    let groups = preview::group_by_season(&rows, &missing, preview_request.movie);
    let artwork_stale = snapshot.read().as_ref().is_some_and(|(f, _)| {
        f.name != form_value.name
            || f.year != form_value.year
            || f.source != form_value.source
            || f.season != form_value.season
            || f.movie != form_value.movie
            || f.key != form_value.key
            || f.custom != form_value.custom
            || f.options.selected_id != form_value.options.selected_id
            || f.options.tmdb_key != form_value.options.tmdb_key
    });
    let artwork = if artwork_stale {
        Default::default()
    } else {
        preview_show.artwork.clone()
    };
    let poster_title = if artwork_stale || preview_show.name.is_empty() {
        "Poster preview".into()
    } else {
        preview_show.year.map_or_else(
            || preview_show.name.clone(),
            |year| format!("{} ({year})", preview_show.name),
        )
    };
    let seasons: std::collections::BTreeSet<_> = if artwork_stale {
        Default::default()
    } else {
        preview_show.episodes.iter().map(|e| e.season).collect()
    };
    let mut carousel_posters = vec![poster::PosterItem {
        title: poster_title,
        url: artwork.poster.clone(),
    }];
    carousel_posters.extend(seasons.iter().map(|season| poster::PosterItem {
        title: format!("Season {season:02}"),
        url: artwork.seasons.get(season).cloned(),
    }));
    let pending_resume = resume.read().clone();

    if splash_visible() {
        return rsx! { branding::Splash { progress: startup_progress(), onartworkready: move |_| splash_artwork_ready.set(true), oncontinue: move |_| {
            branding::finish_splash();
            splash_visible.set(false);
        } } };
    }

    rsx! {
        style { "{CSS}" }
        if cfg!(any(target_os = "android", target_os = "ios")) { MobileStorageBridge {} }
        main { "data-view":active_panel(), class: if cfg!(any(target_os = "android", target_os = "ios")) { "mobile-app" } else { "desktop-app" },
            onpointermove: move |e| {
                if let Some(start) = drag() {
                    let delta = e.client_coordinates().x - start.x;
                    if start.right { right_width.set((start.width - delta).clamp(260.0, 480.0)); }
                    else { left_width.set((start.width + delta).clamp(280.0, 520.0)); }
                }
            },
            onpointerup: move |_| { if drag().is_some() { drag.set(None); } },
            onpointercancel: move |_| drag.set(None),
            workspace::Titlebar {}
            if cfg!(not(any(target_os="android",target_os="ios"))) {workspace::ResizeEdges {}}
            workspace::Navigation {active:active_panel}
            div { class: "layout", "data-active-panel": active_panel(), style: "--left-width:{left_width}px;--right-width:{right_width}px;",
                section { class: "panel controls left-panel", id: "library-settings", "aria-label": "Library, naming, ordering, profiles and API settings",
                    fieldset { disabled: busy() || confirming() || resume.read().is_some(),
                        div { class: "library-toggle", role: "group", "aria-label": "Library type",
                            button { r#type: "button", class: if form_value.movie { "" } else { "selected" },
                                "aria-pressed": (!form_value.movie).to_string(),
                                onclick: move |_| {let mut f=form.write(); f.movie=false; f.options.selected_id.clear(); f.options.poster_override=None; f.options.season_overrides.clear();}, "TV Shows" }
                            button { r#type: "button", class: if form_value.movie { "selected" } else { "" },
                                "aria-pressed": form_value.movie.to_string(),
                                onclick: move |_| {let mut f=form.write(); f.movie=true; f.options.selected_id.clear(); f.options.poster_override=None; f.options.season_overrides.clear();}, "Movies" }
                        }
                        if cfg!(any(target_os = "android", target_os = "ios")) {
                            ads::NativeAdSlot {}
                        }
                        section { class: "settings-group connect-group",
                        h2 { "Connect your library" }
                        label { "Storage"
                            select { value: if connection_value.remote { "remote" } else { "local" },
                                onchange: move |e| connection.write().remote = e.value() == "remote",
                                option { value: "local", "This device" }
                                option { value: "remote", "Remote server" }
                            }
                        }
                        if connection_value.remote {
                            label { "Server URL" input { r#type: "url", value: "{connection_value.url}",
                                oninput: move |e| connection.write().url = e.value() } }
                            label { "Bearer token" input { r#type: "password", value: "{connection_value.token}", autocomplete: "off",
                                oninput: move |e| connection.write().token = e.value() } }
                            p { class: "hint", "Choose a subfolder relative to the server's configured media root. Leave blank for the root." }
                        } else {
                            if cfg!(any(target_os = "android", target_os = "ios")) {
                                p { class: "hint", "Choose a media folder using the system picker. Only that folder is shared with Media Manager." }
                                p { class: "hint", if connection_value.root.is_empty() { "No folder selected" } else { "Media folder selected" } }
                            } else {
                                label { "Media root" input { value: "{connection_value.root}", placeholder: "Select a folder or enter its absolute path",
                                    oninput: move |e| connection.write().root = e.value() } }
                            }
                        }
                        label { "Subfolder (optional)" input { value: "{form_value.folder}", placeholder: "Example: Breaking Bad/Season 01",
                            oninput: move |e| form.write().folder = e.value() } }
                        if !connection_value.remote {
                            if cfg!(any(target_os = "android", target_os = "ios")) {
                                button { class: "secondary", onclick: move |_| {
                                    #[cfg(any(target_os = "android", target_os = "ios"))]
                                    spawn(async move {
                                        match mobile_storage::pick().await {
                                            Ok(root) => { connection.write().root = root; message.set("Media folder selected. Build a new preview.".into()); }
                                            Err(error) => message.set(error.to_string()),
                                        }
                                    });
                                }, "Choose media folder" }
                            } else {
                                button { class: "secondary", onclick: move |_| {
                                    #[cfg(all(feature = "desktop", not(any(target_os = "android", target_os = "ios"))))]
                                    {
                                        let parent = dioxus::desktop::window();
                                        spawn(async move {
                                            if let Some(folder) = rfd::AsyncFileDialog::new()
                                                .set_parent(parent.window.as_ref())
                                                .set_title("Choose your media folder")
                                                .pick_folder().await {
                                                connection.write().root = folder.path().to_string_lossy().into();
                                            }
                                        });
                                    }
                                }, "Browse folders" }
                            }
                        }
                        }
                        div {class:"devices-group",library_tools::Pairing {connection,form,busy,message}}
                        section { class: "settings-group api-group",
                            h2 { "API settings" }
                            if !form_value.movie {
                            label { "Episode source"
                                select { value: "{form_value.source}", onchange: move |e| { let mut f=form.write(); f.source=e.value(); f.options.selected_id.clear(); f.options.poster_override=None; f.options.season_overrides.clear(); },
                                    option { value: "tvmaze", "TVMaze" }
                                    option { value: "omdb", "OMDb" }
                                    option { value: "custom", "Custom TVMaze-compatible API" }
                                    option { value: "manual", "Manual titles" }
                                    option { value: "batch", "Batch sequence" }
                                    option { value: "web", "Episode guide URL" }
                                }
                            }
                            div { class: "provider-settings",
                            h3 { "Provider configuration" }
                            if form_value.source == "tvmaze" { p { class: "hint", "TVMaze is ready to use. No API key required." } }
                            if matches!(form_value.source.as_str(), "manual" | "batch" | "web") { p { class: "hint", "Configure titles, templates or a guide URL in Naming details." } }
                            if form_value.source == "custom" {
                                label { "API base URL" input { value: "{form_value.custom}", placeholder: "https://…",
                                    oninput: move |e| {let mut f=form.write();f.custom=e.value();f.options.selected_id.clear();f.options.poster_override=None;f.options.season_overrides.clear();} } }
                            }
                            if form_value.source == "omdb" || form_value.source == "custom" {
                                label { "API key" input { r#type: "password", value: "{form_value.key}", autocomplete: "off",
                                    oninput: move |e| form.write().key = e.value() } }
                            }
                            }
                        }
                            else {
                                label {"Movie source" select {value:if form_value.source=="omdb" {"omdb"}else{"manual"},onchange:move|e|{let mut f=form.write();f.source=e.value();f.options.selected_id.clear();},option {value:"manual","Entered title and year"} option {value:"omdb","OMDb"}}}
                                if form_value.source=="omdb" {label {"OMDb API key" input {r#type:"password",value:"{form_value.key}",oninput:move|e|form.write().key=e.value()}}}
                            }
                            label {"TMDB artwork API key (optional)" input {r#type:"password",value:"{form_value.options.tmdb_key}",autocomplete:"off",oninput:move|e|form.write().options.tmdb_key=e.value()}}
                            p {class:"hint","Adds verified TMDB artwork using the matched IMDb ID. Different season orders require an explicit poster choice."}
                        }
                        section { class: "settings-group naming-group",
                        h2 { "Naming details" }
                        label { if form_value.movie { "Movie title" } else { "Series name" }
                            input { value: "{form_value.name}", placeholder: "Enter a title", oninput: move |e| { let mut f=form.write(); f.name=e.value(); f.options.selected_id.clear(); f.options.poster_override=None; f.options.season_overrides.clear(); } }
                        }
                        div { class: "pair",
                            label { "Year (optional)" input { r#type: "number", min: "1800", max: "9999", value: "{form_value.year}",
                                oninput: move |e| { let mut f=form.write(); f.year=e.value(); f.options.selected_id.clear(); f.options.poster_override=None; f.options.season_overrides.clear(); } } }
                            if !form_value.movie {
                                label { "Season (optional)" input { r#type: "number", min: "0", value: "{form_value.season}",
                                    placeholder: if matches!(form_value.source.as_str(), "manual" | "batch") { "1" } else { "All seasons" },
                                    oninput: move |e| form.write().season = e.value() } }
                            }
                        }
                        if !form_value.movie {
                            p { class: "hint",
                                if matches!(form_value.source.as_str(), "manual" | "batch") {
                                    "Leave blank to use Season 1 for manual titles or batch numbering."
                                } else {
                                    "Leave blank to include all seasons. Files are matched using their season folder names."
                                }
                            }
                        }
                        if matches!(form_value.source.as_str(),"tvmaze"|"omdb"|"custom") && (!form_value.movie || form_value.source=="omdb") {
                            library_tools::MatchPicker {form,connection,busy,message}
                        }
                        if !form_value.movie {
                            if form_value.source == "manual" {
                                label { "Episode titles, one per line" textarea { rows: "6", value: "{form_value.titles}",
                                    oninput: move |e| form.write().titles = e.value() } }
                            }
                            if form_value.source == "batch" {
                                label { "Naming template" input { value: "{form_value.template}",
                                    oninput: move |e| form.write().template = e.value() } }
                                p { class: "hint", "Tokens: {{Series Name}}, {{Season}}, {{Season:02d}}, {{Episode}}, {{Episode:02d}}" }
                            }
                            if form_value.source == "web" {
                                label { "Episode guide URL" input { r#type: "url", value: "{form_value.guide}", placeholder: "https://en.wikipedia.org/…",
                                    oninput: move |e| form.write().guide = e.value() } }
                                p { class: "hint", "Supports season headings with summary cells and simple numbered tables." }
                            }
                        }
                        }
                        section { class: "settings-group ordering-group",
                            h2 { "Ordering" }
                            label { "Naming format"
                                select { value: if form_value.jellyfin { "jellyfin" } else if form_value.plex { "plex" } else { "none" },
                                    onchange: move |e| {
                                        let value = e.value();
                                        let mut current = form.write();
                                        current.plex = value == "plex";
                                        current.jellyfin = value == "jellyfin";
                                    },
                                    option { value: "none", "None" }
                                    option { value: "plex", if form_value.movie { "Plex movie naming" } else { "Plex episode naming" } }
                                    option { value: "jellyfin", "Jellyfin naming" }
                                }
                            }
                            p {class:"hint",if form_value.movie {if form_value.plex || form_value.jellyfin {"Example: Movie (2020).mkv"}else{"Example: Movie.mkv"}}else if form_value.jellyfin {"Example: Series (2020) S01E01 - Pilot.mkv"}else if form_value.plex {"Example: Series (2020) - S01E01 - Pilot.mkv"}else {"Example: S01E01 - Pilot.mkv"}}
                            library_tools::NumberingOptions {form}
                        }
                        section { class: "settings-group profiles-group",
                            h2 { "Profiles" }
                            library_tools::Profiles {form,message}
                        }
                        div { class: "settings-footer",
                        button { class: "primary", disabled: busy(), onclick: move |_| {
                            let f = form.read().clone();
                            let c = connection.read().clone();
                            let request = match f.request() {
                                Ok(r) => r,
                                Err(e) => { message.set(e.to_string()); return; }
                            };
                            busy.set(true); message.set("Building preview…".into()); plan.set(None); selected.set(None); consumed.set(false);
                            spawn(async move {
                                match c.preview(&request).await {
                                    Ok(p) => { plan.set(Some(p)); snapshot.set(Some((f, c))); message.set("Preview ready. Review every destination before applying.".into()); active_panel.set("posters"); }
                                    Err(e) => message.set(format!("{e:#}")),
                                }
                                busy.set(false);
                            });
                        }, if busy() { "Working…" } else { "Build rename preview" } }
                        p {class:"hint",role:"status","{message}"}
                        }
                    }
                }
                div { class: "resize-handle left-resizer", role: "separator", tabindex: "0", "aria-orientation": "vertical", "aria-label": "Resize library settings", "aria-controls": "library-settings",
                    "aria-valuemin": "280", "aria-valuemax": "520", "aria-valuenow": "{left_width}",
                    onpointerdown: move |e| { e.prevent_default(); drag.set(Some(PanelDrag { right: false, x: e.client_coordinates().x, width: left_width() })); },
                    ondoubleclick: move |_| left_width.set(340.0),
                    onkeydown: move |e| match e.key() {
                        Key::ArrowLeft => { e.prevent_default(); left_width.set((left_width() - 16.0).max(280.0)); },
                        Key::ArrowRight => { e.prevent_default(); left_width.set((left_width() + 16.0).min(520.0)); },
                        _ => (),
                    }
                }
                div { class: "center-column",
                section { class: "panel preview", id: "rename-preview",
                    div {class:"health-tools",library_tools::HistoryHealth {connection,form,plan,busy,message,consumed,changed}}
                    div { class: "preview-heading", div {p {class:"eyebrow","YOUR WORKSPACE"} h2 { "Media library" }} span { class: "badge", "{ready} ready to rename" } }
                    div {class:"library-search",workspace::Icon {name:"search"} input {r#type:"search",value:"{search}",placeholder:"Search files…","aria-label":"Search library files",oninput:move|e|search.set(e.value())}}

                    p { class: "hint",match preview_request.options.numbering {
                        tv_manager_core::Numbering::Sequence=>"Files are paired in natural filename order. Review carefully if episodes are missing.",
                        tv_manager_core::Numbering::Detected=>"Episode numbers are read from filenames. Custom mappings take precedence. Review every match.",
                        tv_manager_core::Numbering::Absolute=>"Absolute numbers map to the provider's episode order. Confirm anime ordering and specials before applying."
                    }}
                    p { role: "status", class: "status", "{message}" }
                    if !persistence_error().is_empty() { p { role: "alert", class: "notice", "{persistence_error}" } }
                    if changed { p { class: "notice", "Settings changed. Build a new preview before applying." } }
                    if conflict { p { class: "notice", "Resolve destination conflicts, then build a new preview." } }
                    if groups.is_empty() {
                        div { class: "empty",workspace::Icon {name:"library"} h3 { "A little order. A better library." } p { "Connect your media folder, choose your naming preferences, and review changes before applying." } button {class:"primary",onclick:move|_|active_panel.set("settings"),"Connect a library"} }
                    } else {
                        for group in &groups {
                        details { class: "season-group", open: true, key: "{group.title(preview_request.movie)}",
                            summary { class: "season-heading",
                                span { "{group.title(preview_request.movie)}" }
                                span { class: "season-count", "{group.file_count()} files · {group.missing.len()} missing" }
                            }
                        div { class: "table-wrap", table {
                            thead { tr { th { "Current file" } th { "Proposed filename" } th { "Status" } th { class: "actions-heading", "Actions" } } }
                            tbody {
                                for (index, row) in group.rows.iter().cloned() {
                                    if search().is_empty() || format!("{} {}",row.source.display(),row.target.display()).to_lowercase().contains(&search().to_lowercase()) {
                                    tr { key: "{index}",class:if selected()==Some(index){"media-row selected"}else{"media-row"},tabindex:if row.directory {"-1"}else{"0"},"aria-selected":(selected()==Some(index)).to_string(),
                                        onclick:move|_|{if !row.directory {selected.set(Some(index));active_panel.set("inspector");}},
                                        onkeydown:move|e|{if !row.directory && (e.key()==Key::Enter || e.key()==Key::Character(" ".into())){e.prevent_default();selected.set(Some(index));active_panel.set("inspector");}},
                                        td { title: "{row.source.display()}", "{row.source.file_name().unwrap_or_default().to_string_lossy()}" }
                                        td { title: "{row.target.display()}", "{row.target.file_name().unwrap_or_default().to_string_lossy()}" }
                                        td { span { class: if matches!(row.status, TaskStatus::Conflict | TaskStatus::Failed) { "error" } else { "" }, "{row.status:?}" }
                                            if !row.message.is_empty() { small { "{row.message}" } }
                                        }
                                        td { class: "file-actions",
                                            if !row.directory {
                                                button { r#type: "button", class: "row-action", title: "Edit proposed filename", "aria-label": "Edit proposed filename",
                                                    disabled: busy() || changed || consumed(), onclick: move |e| {e.stop_propagation();selected.set(Some(index));active_panel.set("inspector");},
                                                    svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8",
                                                        path { d: "m16 3 5 5-12 12-6 1 1-6L16 3Z" } path { d: "m13 6 5 5" }
                                                    }
                                                }
                                                button { r#type: "button", class: "row-action", title: "Preview media file", "aria-label": "Preview media file",
                                                    disabled: busy() || changed || !tv_manager_core::rename::is_video(&row.source),
                                                    onclick: move |e| {e.stop_propagation();selected.set(Some(index));active_panel.set("inspector");},
                                                    svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8",
                                                        path { d: "m8 4 12 8-12 8V4Z" }
                                                    }
                                                }
                                            } else { span { class: "hint", "—" } }
                                        }
                                    }
                                }
                                }
                                for episode in &group.missing {
                                    tr { class: "missing-row", key: "missing-{episode.season}-{episode.number}",
                                        td { "Missing file" }
                                        td { "{episode_name(&preview_request, &preview_show, episode).unwrap_or_else(|_| episode.name.clone())}" }
                                        td { "Missing" small { "S{episode.season:02}E{episode.number:02} has no matching file" } }
                                        td { class: "file-actions", span { class: "hint", "—" } }
                                    }
                                }
                            }
                        } }
                        }
                        }
                    }
                    if !posters.is_empty() {
                        details { class: "season-group", open: true,
                            summary { class: "season-heading", "Poster downloads" span { class: "season-count", "{poster_count} ready" } }
                            div { class: "poster-results",
                                for (index, poster) in posters.iter().enumerate() {
                                    div { key: "poster-{index}", class: "poster-result",
                                        strong { "{poster.label}" }
                                        span { class: if poster.status == TaskStatus::Failed { "error" } else { "" }, "{poster.status:?}" }
                                        small { "{poster.target.display()}" }
                                        if !poster.message.is_empty() { small { "{poster.message}" } }
                                    }
                                }
                            }
                        }
                    }
                }
            if plan.read().is_some() { footer { class: "apply-bar", role: "toolbar", "aria-label": "Library actions",
                div {
                    strong { "{ready} renames · {poster_count} poster downloads ready" }
                    p { class: "hint", "Review before applying. Rename history keeps original names for Undo." }
                }
                div { class: "apply-actions",
                    button { class: "secondary single-column-only", r#type: "button", "aria-controls": "poster-naming-settings", onclick: move |_| active_panel.set("posters"), "← Back" }
                    button { class: "primary", disabled: !can_apply, onclick: move |_| confirming.set(true),
                        if poster_count > 0 { "Apply renames & posters" } else if ready==1 {"Rename 1 file"} else { "Rename {ready} files" }
                    }
                }
            } }
                }
                div { class: "resize-handle right-resizer", role: "separator", tabindex: "0", "aria-orientation": "vertical", "aria-label": "Resize preview sidebar", "aria-controls": "poster-naming-settings",
                    "aria-valuemin": "260", "aria-valuemax": "480", "aria-valuenow": "{right_width}",
                    onpointerdown: move |e| { e.prevent_default(); drag.set(Some(PanelDrag { right: true, x: e.client_coordinates().x, width: right_width() })); },
                    ondoubleclick: move |_| right_width.set(300.0),
                    onkeydown: move |e| match e.key() {
                        Key::ArrowLeft => { e.prevent_default(); right_width.set((right_width() + 16.0).min(480.0)); },
                        Key::ArrowRight => { e.prevent_default(); right_width.set((right_width() - 16.0).max(260.0)); },
                        _ => (),
                    }
                }
                section { class: "panel controls right-panel", id: "poster-naming-settings", "aria-label": "Show and season posters",
                    workspace::Inspector {selected,plan,connection:snapshot.read().as_ref().map(|(_,c)|c.clone()).unwrap_or_else(||connection_value.clone()),disabled:changed || consumed() || confirming(),busy,message}
                    div {class:"artwork-heading",h2 {"Artwork"}}
                    fieldset { disabled: busy() || confirming() || resume.read().is_some(),
                        section { class: "poster-box", "aria-label": "Show and season posters",
                            poster::PosterCarousel { key: "{preview_show.name}-{preview_request.season:?}-{artwork_stale}", posters: carousel_posters }
                            if plan.read().is_none() || artwork_stale {
                                p { class: "hint", "Find a show with Build rename preview to load its artwork." }
                            } else if artwork.poster.is_none() && artwork.seasons.is_empty() {
                                p { class: "hint", "No posters available from this source." }
                            }
                            if !artwork_stale && !seasons.is_empty() && seasons.iter().any(|season| !artwork.seasons.contains_key(season)) {
                                p { class: "hint", "The selected provider has no poster for some seasons. Those seasons display a placeholder." }
                            }
                            div { class: "poster-download-option",
                                label { class: "check", input { r#type: "checkbox", checked: form_value.download_posters,
                                    onchange: move |e| form.write().download_posters = e.checked() } "Download posters" }
                                button { r#type: "button", class: "icon-button info-button", title: "About poster downloads",
                                    "aria-label": "About poster downloads", "aria-expanded": show_poster_help().to_string(),
                                    "aria-controls": "poster-download-help", onclick: move |_| show_poster_help.toggle(),
                                    "ⓘ"
                                }
                            }
                            if show_poster_help() {
                                p { id: "poster-download-help", class: "hint poster-help", "Save available posters when applying the preview. Existing images are kept." }
                            }
                            if matches!(preview_request.source, Source::TvMaze) && artwork.poster.is_some() {
                                a { class: "poster-credit", href: "https://www.tvmaze.com", target: "_blank", rel: "noopener noreferrer", "Artwork via TVMaze" }
                            }
                        }
                    }
                    library_tools::ArtworkPicker {form,connection,plan,snapshot,busy,message}
                    if cfg!(any(target_os = "android", target_os = "ios")) {
                        div {class:"poster-ad",ads::NativeAdSlot { posters: true }}
                    }
                    nav { class: "poster-navigation single-column-only", "aria-label": "Preview steps",
                        button { class: "secondary", r#type: "button", disabled: busy() || confirming(), "aria-controls": "library-settings", onclick: move |_| active_panel.set("settings"), "← Back" }
                        button { class: "primary", r#type: "button", disabled: busy() || confirming(), "aria-controls": "rename-preview", onclick: move |_| active_panel.set("preview"), "Next →" }
                    }
                }
            }
            if drag().is_some() {
                div { class: "resize-capture", onpointerleave: move |_| drag.set(None) }
            }
            if let Some(previous) = pending_resume {
                div { class: "overlay",
                    section { class: "dialog panel resume-dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": "resume-title",
                        h2 { id: "resume-title", "Pick up where you left off?" }
                        p { "Restore your previous library, naming options, API settings, poster preferences and panel widths?" }
                        div { class: "resume-summary",
                            strong { if previous.form.name.is_empty() { "Previous workspace" } else { "{previous.form.name}" } }
                            small { if previous.connection.remote { "Remote library: {previous.connection.url}" } else { "Library: {previous.connection.root}" } }
                        }
                        p { class: "hint", "Your file preview will be refreshed. Renames and poster downloads still require you to apply the preview." }
                        if previous.credentials_omitted { p { class: "hint", "API keys and server tokens need to be entered again on this platform." } }
                        div { class: "pair",
                            button { class: "secondary", onclick: move |_| {
                                resume.set(None);
                                persistence_ready.set(true);
                                message.set("Started a new workspace.".into());
                                if session::clear().is_err() { persistence_error.set("Could not discard the saved workspace. New settings will replace it when saved.".into()); }
                            }, "Start fresh" }
                            button { class: "primary", autofocus: true, onclick: move |_| {
                                let f = previous.form.clone();
                                let c = previous.connection.clone();
                                form.set(f.clone()); connection.set(c.clone());
                                left_width.set(previous.left_width); right_width.set(previous.right_width);
                                resume.set(None); persistence_ready.set(true);
                                message.set(if previous.credentials_omitted { "Settings restored. Enter your API key or server token again to rebuild the preview.".into() }
                                    else { "Previous workspace settings restored.".into() });
                                if previous.had_preview && !previous.credentials_omitted {
                                    if let Ok(request) = f.request() {
                                        busy.set(true);
                                        message.set("Settings restored. Refreshing your library preview…".into());
                                        spawn(async move {
                                            match c.preview(&request).await {
                                                Ok(p) => { plan.set(Some(p)); snapshot.set(Some((f, c))); message.set("Workspace resumed and preview refreshed.".into()); active_panel.set("posters"); }
                                                Err(e) => message.set(format!("Settings restored. Could not refresh the preview: {e:#}")),
                                            }
                                            busy.set(false);
                                        });
                                    }
                                }
                            }, "Resume workspace" }
                        }
                    }
                }
            }
            if confirming() {
                div { class: "overlay",
                    section { class: "dialog panel", role: "dialog", "aria-modal": "true", "aria-labelledby": "confirm-title",
                        h2 { id: "confirm-title", "Apply this rename plan?" }
                        p { "This will rename {ready} files in the selected library. Files with no episode match will be skipped." }
                        if poster_count > 0 { p { "Also download {poster_count} posters to the library. Existing images will be kept." } }
                        div { class: "pair",
                            button { class: "secondary", onclick: move |_| confirming.set(false), "Keep reviewing" }
                            button { class: "primary", onclick: move |_| {
                                let Some(p) = plan.read().clone() else { return; };
                                let c = connection.read().clone();
                                confirming.set(false); busy.set(true); consumed.set(true); message.set("Applying changes…".into());
                                spawn(async move {
                                    match c.apply(p).await {
                                        Ok(p) => {
                                            let success = p.tasks.iter().filter(|t| t.status == TaskStatus::Applied).count();
                                            let failures = p.tasks.iter().filter(|t| t.status == TaskStatus::Failed).count();
                                            let downloaded = p.posters.iter().filter(|t| t.status == TaskStatus::Applied).count();
                                            let poster_failures = p.posters.iter().filter(|t| t.status == TaskStatus::Failed).count();
                                            notifications::completed(format!("{success} renamed; {failures} rename failures. {downloaded} posters saved; {poster_failures} poster failures."));
                                            message.set(format!("Finished: {success} renamed, {failures} rename failures; {downloaded} posters downloaded, {poster_failures} poster failures. Review results below."));
                                            plan.set(Some(p));
                                        }
                                        Err(e) => message.set(format!("{e:#}. Build a new preview before retrying.")),
                                    }
                                    busy.set(false);
                                });
                            }, "Apply renames" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod form_tests {
    use super::*;

    #[test]
    fn optional_season_selects_all_but_preserves_explicit_seasons_and_specials() {
        let mut form = Form::default();
        assert_eq!(form.request().unwrap().season, None);
        form.season = " 2 ".into();
        assert_eq!(form.request().unwrap().season, Some(2));
        form.season = "0".into();
        assert_eq!(form.request().unwrap().season, Some(0));
        form.season = "-1".into();
        assert!(form.request().is_err());
    }

    #[test]
    fn untagged_manual_sources_default_to_one_and_movies_ignore_the_season() {
        for source in ["manual", "batch"] {
            let mut form = Form {
                source: source.into(),
                ..Default::default()
            };
            assert_eq!(form.request().unwrap().season, Some(1));
            form.movie = true;
            assert_eq!(form.request().unwrap().season, None);
        }
    }
}
