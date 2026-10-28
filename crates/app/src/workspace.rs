use crate::{backend::Connection, branding, media_player};
use dioxus::prelude::*;
use tv_manager_core::rename::RenamePlan;

#[component]
pub fn Icon(name: String) -> Element {
    let path = match name.as_str() {
        "welcome" => "m3 11 9-8 9 8 M5 10v11h5v-7h4v7h5V10",
        "check" => "m5 12 4 4L19 6",
        "library" => "M4 3h6v18H4z M14 3h6v18h-6z M4 7h6 M14 7h6 M4 17h6 M14 17h6",
        "folder" => "M3 7V4h6l3 3h9v13H3V7Z",
        "naming" => "m4 17 12-12 3 3L7 20H4v-3Z M13 8l3 3 M13 20h7",
        "ordering" => "M8 4h12 M8 12h9 M8 20h6 M3 3v18 M1 18l2 3 2-3",
        "profiles" => "M5 3h14v18l-7-4-7 4V3Z",
        "posters" => "M3 3h18v18H3z M3 16l5-5 4 4 3-3 6 6 M16 7h.01",
        "devices" => "M2 3h14v12H2z M6 19h6 M9 15v4 M18 9h4v12h-4V9Z",
        "api" => "m8 6-6 6 6 6 M16 6l6 6-6 6 M14 3l-4 18",
        "settings" => "M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1 1-3Z M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0",
        "health" => "M3 12h4l3-8 4 16 3-8h4",
        "search" => "M21 21l-6-6 M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0",
        "close" => "m6 6 12 12 M6 18 18 6",
        "more" => "M5 12h.01 M12 12h.01 M19 12h.01",
        "play" => "m8 4 12 8-12 8V4Z",
        "chevron-down" => "m6 9 6 6 6-6",
        "back" => "m14 5-7 7 7 7",
        "minimize" => "M5 12h14",
        "maximize" => "M5 5h14v14H5z",
        _ => "M12 3v18 M3 12h18",
    };
    rsx! { svg { width:"20",height:"20",view_box:"0 0 24 24",fill:"none",stroke:"currentColor",stroke_width:"1.6",stroke_linecap:"round",stroke_linejoin:"round","aria-hidden":"true", path {d:path} } }
}

#[component]
pub fn Titlebar(context: String, mut form: Signal<crate::Form>, disabled: bool) -> Element {
    rsx! { header {class:"workspace-titlebar",
        div {class:"titlebar-drag", onmousedown:move|_|drag_window(), ondoubleclick:move|_|window_action("maximize"),
            img {src:branding::icon_uri(),alt:""} strong {"Media Manager"} span {class:"titlebar-context","{context}"}
        }
        div {class:"library-toggle titlebar-media-toggle",role:"group","aria-label":"Workspace media type",
            for (movie,label) in [(false,"TV Shows"),(true,"Movies")] {
                button {r#type:"button",class:if form.read().movie==movie {"selected"}else{""},disabled,
                    "aria-pressed":(form.read().movie==movie).to_string(),onclick:move |_|{
                        if form.peek().movie==movie{return;}
                        let mut f=form.write();f.movie=movie;if movie && !matches!(f.source.as_str(),"tmdb"|"omdb"|"custom"|"manual") {f.source="manual".into();} else if !movie && f.source=="tmdb" {f.source="tvmaze".into();}f.options.selected_id.clear();f.options.poster_override=None;f.options.season_overrides.clear();
                    },"{label}"
                }
            }
        }
        span {class:"titlebar-separator","aria-hidden":"true","|"}
        div {class:"window-actions",
            button {title:"Minimize","aria-label":"Minimize window",onclick:move|_|window_action("minimize"),Icon {name:"minimize"}}
            button {title:"Maximize or restore","aria-label":"Maximize or restore window",onclick:move|_|window_action("maximize"),Icon {name:"maximize"}}
            button {class:"window-close",title:"Close","aria-label":"Close window",onclick:move|_|window_action("close"),Icon {name:"close"}}
        }
    } }
}
fn drag_window() {
    #[cfg(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    dioxus::desktop::window().drag();
}
fn window_action(action: &str) {
    #[cfg(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    {
        let window = dioxus::desktop::window();
        match action {
            "minimize" => window.set_minimized(true),
            "maximize" => window.toggle_maximized(),
            "close" => window.close(),
            _ => (),
        }
    }
    #[cfg(not(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    )))]
    let _ = action;
}

const DESTINATIONS: [(&str, &str, &str); 5] = [
    ("welcome", "welcome", "Get Started"),
    ("naming", "naming", "Details"),
    ("preview", "library", "Library"),
    ("health", "health", "Health"),
    ("settings", "settings", "Settings"),
];

#[component]
pub fn ResizeEdges() -> Element {
    // Tao does not implement native drag-resize on macOS. Pointer capture keeps
    // borderless resizing consistent across Windows, macOS and Linux.
    let mut drag = use_signal(|| None::<(String, f64, f64, f64, f64, f64, f64)>);
    rsx! {
        for edge in ["n","s","e","w","ne","nw","se","sw"] {
            div {class:"window-edge edge-{edge}",
                onpointerdown:move|event|{
                    event.prevent_default();
                    #[cfg(all(feature="desktop",not(any(target_os="android",target_os="ios"))))]
                    {
                        let window=dioxus::desktop::window(); let scale=window.scale_factor();
                        let size=window.inner_size().to_logical::<f64>(scale);
                        let position=window.outer_position().ok().map(|p|p.to_logical::<f64>(scale));
                        if let Some(position)=position {
                            drag.set(Some((edge.into(),event.screen_coordinates().x,event.screen_coordinates().y,size.width,size.height,position.x,position.y)));
                            let id=event.pointer_id();
                            let _=document::eval(&format!("document.querySelector('.edge-{edge}').setPointerCapture({id});"));
                        }
                    }
                },
                onpointermove:move|event|{
                    #[cfg(all(feature="desktop",not(any(target_os="android",target_os="ios"))))]
                    if let Some((edge,x,y,w,h,left,top))=drag() {
                        let dx=event.screen_coordinates().x-x;let dy=event.screen_coordinates().y-y;
                        let width=if edge.contains('e'){w+dx}else if edge.contains('w'){w-dx}else{w}.max(480.0);
                        let height=if edge.contains('s'){h+dy}else if edge.contains('n'){h-dy}else{h}.max(400.0);
                        let window=dioxus::desktop::window();
                        if edge.contains('w') || edge.contains('n') {window.set_outer_position(dioxus::desktop::LogicalPosition::new(if edge.contains('w'){left+w-width}else{left},if edge.contains('n'){top+h-height}else{top}));}
                        window.set_inner_size(dioxus::desktop::LogicalSize::new(width,height));
                    }
                    #[cfg(not(all(feature="desktop",not(any(target_os="android",target_os="ios")))))]
                    let _=event;
                },
                onpointerup:move|_|drag.set(None),onpointercancel:move|_|drag.set(None),
            }
        }
    }
}

#[component]
pub fn ConnectionTabs(mut connection: Signal<Connection>) -> Element {
    let remote = connection.read().remote;
    rsx! {div {class:"connection-tabs",role:"tablist","aria-label":"Library connection",
        for (is_remote,id,icon,label) in [(false,"local","folder","Local"),(true,"remote","devices","Remote")] {
            button {id:"connection-{id}-tab",class:if remote==is_remote {"connection-tab selected"}else{"connection-tab"},r#type:"button",role:"tab",
                "aria-selected":(remote==is_remote).to_string(),"aria-controls":"connection-{id}-panel",tabindex:if remote==is_remote {"0"}else{"-1"},
                onclick:move|_|connection.write().remote=is_remote,
                onkeydown:move|e|{
                    let next=match e.key(){Key::ArrowLeft|Key::Home=>false,Key::ArrowRight|Key::End=>true,_=>return};
                    e.prevent_default();connection.write().remote=next;
                    let id=if next {"remote"}else{"local"};
                    let _=document::eval(&format!("document.getElementById('connection-{id}-tab').focus();"));
                },Icon {name:icon} span {"{label}"}
            }
        }
    }}
}

#[component]
pub fn PageSpinner(label: String) -> Element {
    rsx! {div {class:"page-loading",role:"status","aria-live":"polite",
        div {class:"page-loading-badge",span {class:"page-loading-spinner","aria-hidden":"true"} span {"{label}"}}
    }}
}

#[component]
pub fn LibraryTabs(mut active: Signal<&'static str>) -> Element {
    rsx! {div {class:"library-toggle library-tabs",role:"tablist","aria-label":"Library views",
        for (view,label,panel) in [("preview","Library","rename-preview"),("posters","Art Work","poster-naming-settings")] {
            button {r#type:"button",role:"tab",class:if active()==view {"selected"}else{""},
                "aria-selected":(active()==view).to_string(),"aria-controls":panel,
                onclick:move |_|active.set(view),
                onkeydown:move |event|match event.key(){
                    Key::ArrowLeft | Key::Home=>{event.prevent_default();active.set("preview");},
                    Key::ArrowRight | Key::End=>{event.prevent_default();active.set("posters");},
                    _=>(),
                },
                "{label}"
            }
        }
    }}
}

#[component]
pub fn Navigation(active: Signal<&'static str>, onnavigate: EventHandler<&'static str>) -> Element {
    let mut expanded = use_signal(|| false);
    rsx! {
        nav {class:"desktop-rail","aria-label":"Workspace navigation",
            for (view,icon,label) in DESTINATIONS {
                button {class:if active()==view || (view=="preview" && matches!(active(),"inspector"|"posters")) {"rail-item active"}else{"rail-item"},title:label,"aria-label":label,"aria-current":if active()==view || (view=="preview" && matches!(active(),"inspector"|"posters")) {"page"}else{"false"},onclick:move|_|onnavigate.call(view),Icon {name:icon} span {"{label}"}}
            }
            span {class:"rail-version","MM"}
        }
        div {class:if expanded(){"mobile-dock expanded"}else{"mobile-dock"},onkeydown:move|e|{if e.key()==Key::Escape {expanded.set(false);}},
            nav {class:"dock-grid","aria-label":"All workspace tools","inert":(!expanded()).then_some(""),
                for (view,icon,label) in DESTINATIONS {
                    button {class:if active()==view || (view=="preview" && matches!(active(),"inspector"|"posters")) {"dock-tool active"}else{"dock-tool"},onclick:move|_|{onnavigate.call(view);expanded.set(false);},Icon {name:icon} span {"{label}"}}
                }
            }
            nav {class:"dock-bar","aria-label":"Mobile navigation",
                for (view,icon,label) in [("welcome","welcome","Get Started"),("naming","naming","Details"),("preview","library","Library")] {
                    button {class:if active()==view || (view=="preview" && matches!(active(),"inspector"|"posters")) {"dock-tab active"}else{"dock-tab"},tabindex:if expanded(){"-1"}else{"0"},"aria-hidden":expanded().to_string(),onclick:move|_|{onnavigate.call(view);expanded.set(false);},Icon {name:icon} span {"{label}"}}
                }
                button {class:"dock-more","aria-label":if expanded(){"Close tools"}else{"Expand tools"},"aria-expanded":expanded().to_string(),onclick:move|_|expanded.toggle(),Icon {name:if expanded(){"close"}else{"add"}}}
            }
        }
    }
}

#[component]
pub fn PreviewPanel(onclose: EventHandler, children: Element) -> Element {
    use_effect(move || {
        let mut bridge = document::eval(include_str!("preview-sheet.js"));
        spawn(async move {
            while let Ok(action) = bridge.recv::<String>().await {
                if action == "dismiss" {
                    onclose.call(());
                }
            }
        });
    });
    use_drop(|| {
        let _ = document::eval("window.mediaManagerPreviewSheet?.cleanup()");
    });
    rsx! {
        button {class:"preview-sheet-backdrop",r#type:"button",tabindex:"-1","aria-label":"Close preview",onclick:move |_|close_preview()}
        section {class:"panel controls right-panel preview-sheet sheet-surface",id:"media-inspector-sidebar",role:"region","aria-label":"Media and artwork preview",tabindex:"-1",
            div {class:"preview-sheet-chrome",
                div {class:"preview-sheet-grip", "aria-label":"Drag down to close preview",
                    span {"aria-hidden":"true"}
                }
                div {class:"preview-sheet-toolbar",
                    button {class:"preview-sheet-back",r#type:"button","aria-label":"Back to library",onclick:move |_|close_preview(),Icon {name:"back"}}
                    button {class:"preview-sheet-close",r#type:"button","aria-label":"Close preview",onclick:move |_|close_preview(),Icon {name:"close"}}
                }
            }
            div {class:"preview-sheet-content",{children}}
        }
    }
}

pub fn close_preview() {
    let _ = document::eval("window.mediaManagerPreviewSheet?.dismiss()");
}

#[component]
pub fn Inspector(
    mut selected: Signal<Option<usize>>,
    mut plan: Signal<Option<RenamePlan>>,
    connection: Connection,
    disabled: bool,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
    onclose: EventHandler,
) -> Element {
    let mut media = use_signal(|| None::<media_player::Media>);
    let mut error = use_signal(String::new);
    let mut draft = use_signal(String::new);
    let mut generation = use_signal(|| 0u64);
    let mut loading = use_signal(|| false);
    let mut buffering = use_signal(|| false);
    let pending =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<dioxus::core::Task>)));
    let cleanup = pending.clone();
    use_drop(move || {
        if let Some(task) = cleanup.borrow_mut().take() {
            task.cancel();
        }
        #[cfg(target_os = "android")]
        let _=document::eval("window.MediaManagerStorage?.postMessage(JSON.stringify({op:'stopMediaPreview',id:'stop-preview'}))");
    });
    media_player::use_media_handler(media);
    let effect_connection = connection.clone();
    use_effect(move || {
        if let Some(task) = pending.borrow_mut().take() {
            task.cancel();
        }
        #[cfg(target_os = "android")]
        let _=document::eval("window.MediaManagerStorage?.postMessage(JSON.stringify({op:'stopMediaPreview',id:'stop-preview'}))");
        let current = plan.read().clone();
        let index = selected();
        let c = effect_connection.clone();
        let next = generation.peek().wrapping_add(1);
        generation.set(next);
        media.set(None);
        error.set(String::new());
        loading.set(false);
        buffering.set(false);
        if let Some((p, index)) = current.zip(index) {
            if let Some(task) = p.tasks.get(index) {
                draft.set(
                    task.target
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                );
                if tv_manager_core::rename::is_video(&task.source) {
                    loading.set(true);
                    *pending.borrow_mut() = Some(spawn(async move {
                        let result = c.preview_media(p, index).await;
                        if *generation.peek() != next {
                            return;
                        }
                        match result {
                            Ok(value) => media.set(Some(value)),
                            Err(e) => error.set(format!("Could not preview this file: {e:#}")),
                        }
                        loading.set(false);
                    }));
                }
            }
        }
    });
    let task = selected().and_then(|i| plan.read().as_ref().and_then(|p| p.tasks.get(i).cloned()));
    rsx! {section {class:"media-inspector","aria-label":"Selected media",
        div {class:"inspector-heading",h2 {"File preview"} if selected().is_some() {button {class:"quiet-icon",title:"Clear selection","aria-label":"Clear selection",onclick:move|_|onclose.call(()),Icon {name:"close"}}}}
        if let Some(task)=task {
            div {class:"video-stage",
                if let Some(value)=media.read().as_ref() {
                    video {key:"{value.id}",src:value.source(),controls:true,autoplay:true,muted:true,playsinline:true,preload:"metadata",onwaiting:move |_|buffering.set(true),onstalled:move |_|buffering.set(true),onplaying:move |_|buffering.set(false),oncanplay:move |_|buffering.set(false),
                        onerror:move|_|{buffering.set(false);error.set("Preview could not be played. Check the connection and file format. MP4 with H.264/AAC is widely supported.".into());}}
                } else {div {class:"video-placeholder",Icon {name:"play"} span {if loading(){"Preparing preview…"}else{"No video preview"}}}}
            }
            if buffering() && error().is_empty() {p {class:"hint",role:"status","Buffering video…"}}
            if !error().is_empty() {p {class:"error",role:"alert","{error}"}}
            p {class:"inspector-filename",title:"{task.source.display()}","{task.source.file_name().unwrap_or_default().to_string_lossy()}"}
            label {"Selected item title" input {value:"{draft}",disabled:disabled || busy(),oninput:move|e|draft.set(e.value()),placeholder:"Enter a title"}}
            p {class:"hint","The extension is kept. Save updates the preview; Apply renames changes the file."}
            button {class:"secondary",disabled:disabled || busy() || draft().trim().is_empty() || task.target.file_stem().and_then(|s|s.to_str())==Some(draft().trim()),onclick:move|_|{
                let Some(index)=selected() else{return;}; let Some(p)=plan.read().clone() else{return;};
                let extension=p.tasks[index].target.extension().map(|e|format!(".{}",e.to_string_lossy())).unwrap_or_default();
                let filename=format!("{}{extension}",draft().trim());
                let c=connection.clone(); busy.set(true); error.set(String::new());
                spawn(async move {
                    match c.edit(p,index,filename).await {
                        Ok(updated)=>{plan.set(Some(updated));message.set("Title updated in the rename preview.".into());}
                        Err(e)=>error.set(format!("Could not save title: {e:#}")),
                    }
                    busy.set(false);
                });
            },"Save title"}
            p {class:"hint playback-note","Preview starts muted. Use the player controls for sound."}
        } else {
            div {class:"video-stage",div {class:"video-placeholder",Icon {name:"play"} span {"Select a media file"}}}
            p {class:"hint","Click a file in your library to preview it and edit its title here."}
        }
    }}
}
