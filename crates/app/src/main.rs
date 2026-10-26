mod ads;
mod backend;
mod branding;
mod notifications;
mod poster;
mod preview;
mod session;

use backend::Connection;
use dioxus::prelude::*;
use tv_manager_core::{
    rename::{episode_name, RenamePlan, TaskStatus},
    PlanRequest, Source,
};

const CSS: &str = include_str!("style.css");

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
    folder: String,
    source: String,
    key: String,
    custom: String,
    titles: String,
    guide: String,
    template: String,
    download_posters: bool,
}
impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            year: String::new(),
            season: String::new(),
            movie: false,
            plex: false,
            folder: String::new(),
            source: "tvmaze".into(),
            key: String::new(),
            custom: String::new(),
            titles: String::new(),
            guide: String::new(),
            template: "{Series Name} - S{Season:02d}E{Episode:02d}".into(),
            download_posters: false,
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
            source,
            download_posters: self.download_posters,
        })
    }
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
    let initial_session = use_hook(|| session::load().map_err(|e| e.to_string()));
    let mut resume = use_signal(|| initial_session.as_ref().ok().cloned().flatten());
    let mut persistence_ready = use_signal(|| resume.read().is_none());
    let mut form = use_signal(Form::default);
    let mut connection = use_signal(Connection::default);
    let mut plan = use_signal(|| None::<RenamePlan>);
    let mut snapshot = use_signal(|| None::<(Form, Connection)>);
    let mut busy = use_signal(|| false);
    let mut message = use_signal(|| {
        if initial_session.is_err() {
            "The previous workspace could not be read. You can start a new workspace.".into()
        } else {
            String::new()
        }
    });
    let mut persistence_error = use_signal(String::new);
    let mut confirming = use_signal(|| false);
    let mut consumed = use_signal(|| false);
    let mut show_poster_help = use_signal(|| false);
    let mut editing = use_signal(|| None::<usize>);
    let mut edit_name = use_signal(String::new);
    let mut edit_error = use_signal(String::new);
    let mut playback = use_signal(|| None::<(String, String)>);
    let mut playback_error = use_signal(String::new);
    let mut left_width = use_signal(|| 340.0f64);
    let mut right_width = use_signal(|| 300.0f64);
    let mut active_panel = use_signal(|| "naming");
    let mut panel_menu_open = use_signal(|| false);
    let mut drag = use_signal(|| None::<PanelDrag>);
    let mut splash_visible = use_signal(|| {
        cfg!(all(
            feature = "desktop",
            not(any(target_os = "android", target_os = "ios"))
        ))
    });
    use_future(move || async move {
        if splash_visible() {
            tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
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
    let can_apply = (ready > 0 || poster_count > 0)
        && !conflict
        && !changed
        && !busy()
        && !consumed()
        && editing().is_none();
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
            || f.source != form_value.source
            || f.season != form_value.season
            || f.movie != form_value.movie
            || f.key != form_value.key
            || f.custom != form_value.custom
    });
    let artwork = if artwork_stale {
        Default::default()
    } else {
        preview_show.artwork.clone()
    };
    let poster_title = if artwork_stale || preview_show.name.is_empty() {
        "Poster preview".into()
    } else {
        preview_show.name.clone()
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
    let active_playback = playback.read().clone();

    if splash_visible() {
        return rsx! { branding::Splash { oncontinue: move |_| {
            branding::finish_splash();
            splash_visible.set(false);
        } } };
    }

    rsx! {
        style { "{CSS}" }
        main {
            onpointermove: move |e| {
                if let Some(start) = drag() {
                    let delta = e.client_coordinates().x - start.x;
                    if start.right { right_width.set((start.width - delta).clamp(260.0, 480.0)); }
                    else { left_width.set((start.width + delta).clamp(280.0, 520.0)); }
                }
            },
            onpointerup: move |_| { if drag().is_some() { drag.set(None); } },
            onpointercancel: move |_| drag.set(None),
            nav { class: "layout-tabs", "aria-label": "Dashboard panels",
                onkeydown: move |e| { if e.key() == Key::Escape {
                    panel_menu_open.set(false);
                    let _ = document::eval("document.getElementById('panel-menu-toggle')?.focus()");
                } },
                button { id: "panel-menu-toggle", class: "panel-menu-toggle", r#type: "button", title: "Switch dashboard panel", "aria-label": "Switch dashboard panel",
                    "aria-controls": "panel-menu", "aria-expanded": panel_menu_open().to_string(), onclick: move |_| panel_menu_open.toggle(),
                    svg { width: "21", height: "21", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
                        path { d: "m9.5 3-.5 2a8 8 0 0 0-1.7 1L5.4 5.4 3 9.6 4.5 11a8 8 0 0 0 0 2L3 14.4l2.4 4.2 1.9-.6a8 8 0 0 0 1.7 1l.5 2h5l.5-2a8 8 0 0 0 1.7-1l1.9.6 2.4-4.2-1.5-1.4a8 8 0 0 0 0-2L21 9.6l-2.4-4.2-1.9.6A8 8 0 0 0 15 5l-.5-2h-5Z" }
                        circle { cx: "12", cy: "12", r: "3" }
                    }
                }
                if panel_menu_open() {
                    button { class: "panel-menu-dismiss", r#type: "button", tabindex: "-1", "aria-label": "Close panel selector", onclick: move |_| panel_menu_open.set(false) }
                    div { id: "panel-menu", class: "panel-menu", "aria-label": "Choose dashboard panel",
                        for (value, title, target) in [("naming", "Naming", "poster-naming-settings"), ("preview", "Preview", "rename-preview"), ("library", "Library & API", "library-settings")] {
                            button { r#type: "button", "aria-controls": target, "aria-pressed": (active_panel() == value).to_string(), onclick: move |_| {
                                active_panel.set(value); panel_menu_open.set(false);
                                let _ = document::eval("document.getElementById('panel-menu-toggle')?.focus()");
                            }, "{title}" }
                        }
                    }
                }
            }
            div { class: "layout", "data-active-panel": active_panel(), style: "--left-width:{left_width}px;--right-width:{right_width}px;",
                section { class: "panel controls left-panel", id: "poster-naming-settings", "aria-label": "Posters and naming details",
            header { class: "app-identity",
                div { class: "brand", img { src: branding::icon_uri(), alt: "Media Manager icon" } }
                div { h1 { "Media Manager" } p { "Organize your library, one clear preview at a time." } }
            }
                    fieldset { disabled: busy() || confirming() || resume.read().is_some() || editing().is_some(),
                        div { class: "library-toggle", role: "group", "aria-label": "Library type",
                            button { r#type: "button", class: if form_value.movie { "" } else { "selected" },
                                "aria-pressed": (!form_value.movie).to_string(),
                                onclick: move |_| form.write().movie = false, "TV Shows" }
                            button { r#type: "button", class: if form_value.movie { "selected" } else { "" },
                                "aria-pressed": form_value.movie.to_string(),
                                onclick: move |_| form.write().movie = true, "Movies" }
                        }
                        section { class: "poster-box", "aria-label": "Show and season posters",
                            poster::PosterCarousel { key: "{preview_show.name}-{preview_request.season:?}-{artwork_stale}", posters: carousel_posters }
                            if plan.read().is_none() || artwork_stale {
                                p { class: "hint", "Find a show with Build rename preview to load its artwork." }
                            } else if artwork.poster.is_none() && artwork.seasons.is_empty() {
                                p { class: "hint", "No posters available from this source." }
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
                        section { class: "settings-group naming-group",
                        h2 { "Naming details" }
                        label { if form_value.movie { "Movie title" } else { "Series name" }
                            input { value: "{form_value.name}", placeholder: "Enter a title", oninput: move |e| form.write().name = e.value() }
                        }
                        div { class: "pair",
                            label { "Year (optional)" input { r#type: "number", min: "1800", max: "9999", value: "{form_value.year}",
                                oninput: move |e| form.write().year = e.value() } }
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
                        label { class: "check", input { r#type: "checkbox", checked: form_value.plex,
                            onchange: move |e| form.write().plex = e.checked() }
                            if form_value.movie { "Plex movie naming (title and year)" } else { "Plex episode naming" }
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
                    }
                }
                div { class: "resize-handle left-resizer", role: "separator", tabindex: "0", "aria-orientation": "vertical", "aria-label": "Resize posters and naming", "aria-controls": "poster-naming-settings",
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
                    div { class: "preview-heading", h2 { "Review & rename" } span { class: "badge", "{ready} ready" } }
                    p { class: "hint", "Files are paired in natural filename order. Review the mapping carefully, especially if episodes are missing." }
                    p { role: "status", class: "status", "{message}" }
                    if !persistence_error().is_empty() { p { role: "alert", class: "notice", "{persistence_error}" } }
                    if changed { p { class: "notice", "Settings changed. Build a new preview before applying." } }
                    if conflict { p { class: "notice", "Resolve destination conflicts, then build a new preview." } }
                    if groups.is_empty() {
                        div { class: "empty", h3 { "Your next chapter starts here" } p { "Connect a folder and build a preview to see proposed filenames." } }
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
                                    tr { key: "{index}",
                                        td { title: "{row.source.display()}", "{row.source.file_name().unwrap_or_default().to_string_lossy()}" }
                                        td { title: "{row.target.display()}", "{row.target.file_name().unwrap_or_default().to_string_lossy()}" }
                                        td { span { class: if matches!(row.status, TaskStatus::Conflict | TaskStatus::Failed) { "error" } else { "" }, "{row.status:?}" }
                                            if !row.message.is_empty() { small { "{row.message}" } }
                                        }
                                        td { class: "file-actions",
                                            if !row.directory {
                                                button { r#type: "button", class: "row-action", title: "Edit proposed filename", "aria-label": "Edit proposed filename",
                                                    disabled: busy() || changed || consumed(), onclick: move |_| {
                                                        if let Some(p) = plan.read().as_ref() {
                                                            edit_name.set(p.tasks[index].target.file_name().unwrap_or_default().to_string_lossy().into());
                                                            edit_error.set(String::new()); editing.set(Some(index));
                                                        }
                                                    },
                                                    svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8",
                                                        path { d: "m16 3 5 5-12 12-6 1 1-6L16 3Z" } path { d: "m13 6 5 5" }
                                                    }
                                                }
                                                button { r#type: "button", class: "row-action", title: "Play media file", "aria-label": "Play media file",
                                                    disabled: busy() || changed || !matches!(row.source.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), "mp4" | "mkv" | "avi"),
                                                    onclick: move |_| {
                                                        let Some(p) = plan.read().clone() else { return; };
                                                        let c = snapshot.read().as_ref().map(|(_, c)| c.clone()).unwrap_or_else(|| connection.read().clone());
                                                        let title = p.tasks[index].source.file_name().unwrap_or_default().to_string_lossy().to_string();
                                                        busy.set(true); message.set("Opening media player…".into()); playback_error.set(String::new());
                                                        spawn(async move {
                                                            match c.play(p, index).await {
                                                                Ok(Some(url)) => { playback.set(Some((title, url))); message.set("Remote player opened.".into()); }
                                                                Ok(None) => message.set("Opened in your default media player.".into()),
                                                                Err(e) => message.set(format!("Could not play media: {e:#}")),
                                                            }
                                                            busy.set(false);
                                                        });
                                                    },
                                                    svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8",
                                                        path { d: "m8 4 12 8-12 8V4Z" }
                                                    }
                                                }
                                            } else { span { class: "hint", "—" } }
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
            footer { class: "apply-bar", role: "toolbar", "aria-label": "Library actions",
                div {
                    strong { "{ready} renames · {poster_count} poster downloads ready" }
                    p { class: "hint", "Renames affect real files. This preview has no automatic undo." }
                }
                button { class: "primary", disabled: !can_apply, onclick: move |_| confirming.set(true),
                    if poster_count > 0 { "Apply renames & posters" } else { "Rename {ready} files" }
                }
            }
                }
                div { class: "resize-handle right-resizer", role: "separator", tabindex: "0", "aria-orientation": "vertical", "aria-label": "Resize library settings", "aria-controls": "library-settings",
                    "aria-valuemin": "260", "aria-valuemax": "480", "aria-valuenow": "{right_width}",
                    onpointerdown: move |e| { e.prevent_default(); drag.set(Some(PanelDrag { right: true, x: e.client_coordinates().x, width: right_width() })); },
                    ondoubleclick: move |_| right_width.set(300.0),
                    onkeydown: move |e| match e.key() {
                        Key::ArrowLeft => { e.prevent_default(); right_width.set((right_width() + 16.0).min(480.0)); },
                        Key::ArrowRight => { e.prevent_default(); right_width.set((right_width() - 16.0).max(260.0)); },
                        _ => (),
                    }
                }
                section { class: "panel controls right-panel", id: "library-settings", "aria-label": "Library and API settings",
                    fieldset { disabled: busy() || confirming() || resume.read().is_some() || editing().is_some(),
                        section { class: "settings-group",
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
                                p { class: "notice", "Local Android/iOS document access is pending. Use remote server mode in this preview." }
                            } else {
                                label { "Media root" input { value: "{connection_value.root}", placeholder: "Select a folder or enter its absolute path",
                                    oninput: move |e| connection.write().root = e.value() } }
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
                        label { "Subfolder (optional)" input { value: "{form_value.folder}", placeholder: "Example: Breaking Bad/Season 01",
                            oninput: move |e| form.write().folder = e.value() } }
                        }
                        section { class: "settings-group",
                            h2 { "API settings" }
                            if !form_value.movie {
                            label { "Episode source"
                                select { value: "{form_value.source}", onchange: move |e| form.write().source = e.value(),
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
                            if matches!(form_value.source.as_str(), "manual" | "batch" | "web") { p { class: "hint", "Configure titles, templates or a guide URL in the left panel." } }
                            if form_value.source == "custom" {
                                label { "API base URL" input { value: "{form_value.custom}", placeholder: "https://…",
                                    oninput: move |e| form.write().custom = e.value() } }
                            }
                            if form_value.source == "omdb" || form_value.source == "custom" {
                                label { "API key" input { r#type: "password", value: "{form_value.key}", autocomplete: "off",
                                    oninput: move |e| form.write().key = e.value() } }
                            }
                            }
                        }
                            else { p { class: "hint", "Movies use the title and year entered in Naming details." } }
                        }
                        div { class: "settings-footer",
                        if cfg!(any(target_os = "android", target_os = "ios")) {
                            ads::NativeAdSlot {}
                        }
                        button { class: "primary", disabled: busy(), onclick: move |_| {
                            let f = form.read().clone();
                            let c = connection.read().clone();
                            let request = match f.request() {
                                Ok(r) => r,
                                Err(e) => { message.set(e.to_string()); return; }
                            };
                            busy.set(true); message.set("Building preview…".into()); plan.set(None); consumed.set(false);
                            spawn(async move {
                                match c.preview(&request).await {
                                    Ok(p) => { plan.set(Some(p)); snapshot.set(Some((f, c))); message.set("Preview ready. Review every destination before applying.".into()); }
                                    Err(e) => message.set(format!("{e:#}")),
                                }
                                busy.set(false);
                            });
                        }, if busy() { "Working…" } else { "Build rename preview" } }
                        }
                    }
                }
            }
            if drag().is_some() {
                div { class: "resize-capture", onpointerleave: move |_| drag.set(None) }
            }
            if let Some(index) = editing() {
                div { class: "overlay",
                    section { class: "dialog panel", role: "dialog", "aria-modal": "true", "aria-labelledby": "edit-file-title",
                        h2 { id: "edit-file-title", "Edit proposed filename" }
                        p { "This changes the preview. The file is renamed when you apply it." }
                        label { "Filename (keep the extension)" input { value: "{edit_name}", autofocus: true, disabled: busy(),
                            oninput: move |e| edit_name.set(e.value()) } }
                        if !edit_error().is_empty() { p { role: "alert", class: "error", "{edit_error}" } }
                        div { class: "pair",
                            button { class: "secondary", disabled: busy(), onclick: move |_| editing.set(None), "Cancel" }
                            button { class: "primary", disabled: busy(), onclick: move |_| {
                                let Some(p) = plan.read().clone() else { return; };
                                let c = snapshot.read().as_ref().map(|(_, c)| c.clone()).unwrap_or_else(|| connection.read().clone());
                                let filename = edit_name().trim().to_string();
                                busy.set(true); edit_error.set(String::new());
                                spawn(async move {
                                    match c.edit(p, index, filename).await {
                                        Ok(p) => { plan.set(Some(p)); editing.set(None); message.set("Proposed filename updated. Review the preview before applying.".into()); }
                                        Err(e) => edit_error.set(format!("{e:#}")),
                                    }
                                    busy.set(false);
                                });
                            }, if busy() { "Saving…" } else { "Save filename" } }
                        }
                    }
                }
            }
            if let Some((title, url)) = active_playback {
                div { class: "overlay",
                    section { class: "dialog panel playback-dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": "playback-title",
                        h2 { id: "playback-title", "{title}" }
                        video { src: "{url}", controls: true, autoplay: true, playsinline: true, preload: "metadata",
                            onerror: move |_| playback_error.set("This device cannot play this format or the stream is unavailable. Use a compatible player, or refresh the preview if the link expired.".into()) }
                        if !playback_error().is_empty() { p { role: "alert", class: "error", "{playback_error}" } }
                        p { class: "hint", "Remote playback uses a temporary link. Codec support depends on your device." }
                        label { "Stream link (for a compatible player)" input { readonly: true, value: "{url}" } }
                        button { class: "primary", onclick: move |_| playback.set(None), "Close player" }
                    }
                }
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
                                                Ok(p) => { plan.set(Some(p)); snapshot.set(Some((f, c))); message.set("Workspace resumed and preview refreshed.".into()); }
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
