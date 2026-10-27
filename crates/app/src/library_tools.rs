use crate::{backend::Connection, pairing, profiles, Form};
use dioxus::prelude::*;
use tv_manager_core::{rename::RenamePlan, ShowMatch};

#[component]
pub fn MatchPicker(
    mut form: Signal<Form>,
    connection: Signal<Connection>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
    onselected: EventHandler<String>,
) -> Element {
    let mut matches = use_signal(Vec::<ShowMatch>::new);
    let mut searched = use_signal(String::new);
    let value = form.read().clone();
    let search_key = format!(
        "{}|{}|{}|{}|{}",
        value.name, value.year, value.source, value.custom, value.movie
    );
    let stale = searched() != search_key;
    rsx! {
        div {class:"feature-block",
            button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|{
                let request=match form.read().request(){Ok(r)=>r,Err(e)=>{message.set(e.to_string());return;}};
                let c=connection();let key=search_key.clone();busy.set(true);
                spawn(async move {match c.search(&request).await {Ok(items)=>{message.set(if items.is_empty(){"No matches found. Check the title and year.".into()}else{"Choose a matching title before building the preview.".into()});matches.set(items);searched.set(key);},Err(e)=>message.set(e.to_string())}busy.set(false);});
            },"Find matching titles"}
            if !value.options.selected_id.is_empty() {p {class:"hint","Selected provider ID: {value.options.selected_id}"}}
            if !stale {div {class:"match-results",
                for item in matches() {
                    button {r#type:"button",class:"match-result secondary",key:"{item.provider}-{item.id}",disabled:busy(),onclick:{let selected=item.clone();move |_|{
                        let mut f=form.write();f.name=selected.name.clone();f.year=selected.year.map(|y|y.to_string()).unwrap_or_default();f.options.selected_id=selected.id.clone();f.options.poster_override=None;f.options.season_overrides.clear();
                        matches.set(vec![]);message.set("Match selected. The preview refreshes automatically.".into());
                        onselected.call(selected.year.map_or_else(||selected.name.clone(),|year|format!("{} ({year})",selected.name)));
                    }},
                        if let Some(url)=&item.poster {img {src:"{url}",alt:"",loading:"lazy",referrerpolicy:"no-referrer"}}
                        span { {format!("{} ({})",item.name,item.year.map_or_else(|| "Year unknown".into(),|y|y.to_string()))} small {"{item.provider} · {item.kind}"}}
                    }
                }
            }}
        }
    }
}

#[component]
pub fn Profiles(mut form: Signal<Form>, mut message: Signal<String>) -> Element {
    let mut profiles = use_signal(|| profiles::load().unwrap_or_default());
    let mut name = use_signal(String::new);
    let mut selected = use_signal(String::new);
    rsx! {details {class:"feature-block",summary {"Saved naming profiles"}
        label {"Profile" select {value:selected(),onchange:move |e|{
            selected.set(e.value());
            if let Some(profile)=profiles.read().iter().find(|p|p.name==e.value()){profile.apply(&mut form.write());message.set("Naming profile loaded. The preview refreshes automatically.".into());}
        },option {value:"","Choose a profile"} for profile in profiles(){option {value:"{profile.name}","{profile.name}"}}}}
        label {"Profile name" input {value:name(),maxlength:"80",oninput:move|e|name.set(e.value())}}
        div {class:"pair",
            button {r#type:"button",class:"secondary",onclick:move |_|{
                match profiles::Profile::from_form(name(),&form.read()).and_then(|profile|{let mut items=profiles();items.retain(|p|p.name!=profile.name);items.push(profile);profiles::save(&items)?;profiles.set(items);Ok(())}){Ok(())=>message.set("Profile saved.".into()),Err(e)=>message.set(e.to_string())}
            },"Save profile"}
            button {r#type:"button",class:"secondary",disabled:selected().is_empty(),onclick:move |_|{let mut items=profiles();items.retain(|p|p.name!=selected());match profiles::save(&items){Ok(())=>{profiles.set(items);selected.set(String::new());},Err(e)=>message.set(e.to_string())}},"Delete profile"}
        }
        p {class:"hint","Profiles save naming rules and numbering preferences. Library paths and API credentials stay with your connection."}
    }}
}

#[component]
pub fn Pairing(
    connection: Signal<Connection>,
    form: Signal<Form>,
    busy: Signal<bool>,
    message: Signal<String>,
) -> Element {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return rsx! { MobilePairing { connection, form, busy, message } };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = form;
        rsx! { DesktopPairing { connection, busy, message } }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[component]
fn MobilePairing(
    mut connection: Signal<Connection>,
    mut form: Signal<Form>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
) -> Element {
    rsx! { section { class: "settings-group pairing-panel",
        h2 { "Pair a desktop" }
        button { r#type: "button", class: "secondary", disabled: busy(), onclick: move |_| {
            busy.set(true); message.set("Point the camera at the desktop pairing QR.".into());
            spawn(async move {
                let result = async {
                    let invitation = pairing::scan_invitation().await?;
                    let desktop = if invitation.desktop.is_empty() { "Desktop".to_string() } else { invitation.desktop.clone() };
                    Ok::<_, anyhow::Error>((Connection::redeem_pairing(invitation).await?, desktop))
                }.await;
                match result {
                    Ok((value, desktop)) => { connection.set(value); form.write().folder.clear(); message.set(format!("Paired with {desktop}. Open Workspace and enter details to refresh the preview automatically.")); }
                    Err(error) => message.set(format!("Pairing failed: {error:#}")),
                }
                busy.set(false);
            });
        }, "Scan desktop QR code" }
        p { class: "hint", "Scan a QR from Media Manager on your desktop. Local pairing uses the same network; remote pairing uses its HTTPS server." }
    }}
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn lan_address() -> String {
    (|| -> std::io::Result<String> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.connect("192.0.2.1:80")?;
        Ok(socket.local_addr()?.ip().to_string())
    })()
    .unwrap_or_default()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[component]
fn DesktopPairing(
    connection: Signal<Connection>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
) -> Element {
    let mut qr = use_signal(String::new);
    let mut mode = use_signal(|| "local".to_string());
    let mut address = use_signal(lan_address);
    let mut host = use_signal(|| None::<std::sync::Arc<tv_manager_server::LocalHost>>);
    let mut hosted_key = use_signal(String::new);
    let mut expires = use_signal(|| None::<std::time::Instant>);
    let mut remaining = use_signal(|| 0_u64);
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let seconds = expires()
                .map(|deadline| {
                    deadline
                        .saturating_duration_since(std::time::Instant::now())
                        .as_secs()
                })
                .unwrap_or(0);
            remaining.set(seconds);
            if seconds == 0 {
                qr.set(String::new());
            }
        }
    });
    rsx! { section { class: "settings-group pairing-panel",
        h2 { "Pair a mobile device" }
        label { "Connection" select { value: mode(), onchange: move |e| { mode.set(e.value()); qr.set(String::new()); },
            option { value: "local", "Local network" }
            option { value: "remote", "Remote HTTPS server" }
        }}
        if mode() == "local" {
            label { "Desktop LAN address" input { value: address(), placeholder: "192.168.1.10", oninput: move |e| { address.set(e.value()); qr.set(String::new()); } } }
            p { class: "hint", "Choose a media folder in the Local tab first, then use the same network on your phone. Generating a QR starts an encrypted server for that folder. Keep this app open. Allow the app through your private-network firewall if prompted." }
        } else {
            p { class: "hint", "Configure the HTTPS server URL and bearer token above first." }
        }
        button { r#type: "button", class: "secondary", disabled: busy(), onclick: move |_| {
            let original = connection(); let selected_mode = mode(); let ip_text = address();
            qr.set(String::new()); busy.set(true);
            spawn(async move {
                let result = async {
                    let c = if selected_mode == "local" {
                        use base64::Engine;
                        let ip = ip_text.trim().parse::<std::net::IpAddr>().map_err(|_| anyhow::anyhow!("Enter the desktop LAN IP address"))?;
                        anyhow::ensure!(pairing::is_local(ip) && !ip.is_loopback(), "Use a private LAN address reachable from your phone");
                        let key = format!("{}|{ip}", original.root);
                        if host().is_none() || hosted_key() != key {
                            host.set(None);
                            let local = tv_manager_server::host_library(original.root.clone().into(), ip).await?;
                            host.set(Some(std::sync::Arc::new(local))); hosted_key.set(key);
                        }
                        let local = host().ok_or_else(|| anyhow::anyhow!("Local host unavailable"))?;
                        Connection { remote: true, url: local.url.clone(), token: local.token.clone(), certificate: base64::engine::general_purpose::STANDARD.encode(&local.certificate), ..Connection::default() }
                    } else {
                        anyhow::ensure!(original.remote && original.url.starts_with("https://"), "Configure a remote HTTPS server first");
                        original
                    };
                    let invitation = pairing::Invitation { version: 2, url: c.url.clone(), code: c.pairing_code().await?, certificate: c.certificate.clone(), desktop: std::env::var("COMPUTERNAME").or_else(|_|std::env::var("HOSTNAME")).unwrap_or_else(|_|"Media Manager desktop".into()).chars().take(80).collect(), mode: selected_mode };
                    let text = serde_json::to_string(&invitation)?; pairing::Invitation::parse(&text)?;
                    pairing::qr_uri(&text)
                }.await;
                match result {
                    Ok(uri) => { qr.set(uri); expires.set(Some(std::time::Instant::now()+std::time::Duration::from_secs(300))); remaining.set(300); message.set("Scan this QR using Media Manager on your phone.".into()); }
                    Err(error) => message.set(format!("Pairing setup failed: {error:#}")),
                }
                busy.set(false);
            });
        }, "Generate pairing QR" }
        if !qr().is_empty() { img { class: "pairing-qr", src: qr(), alt: "Single-use desktop pairing QR code" } p { class: "hint", "Expires in {remaining} seconds. One phone per QR." } }
        if host().is_some() { button { r#type: "button", class: "secondary", disabled: busy(), onclick: move |_| { host.set(None); qr.set(String::new()); expires.set(None); message.set("Local sharing stopped. Paired phones can no longer access this host.".into()); }, "Stop local sharing" } }
    }}
}

#[component]
pub fn HealthDashboard(
    connection: Signal<Connection>,
    form: Signal<Form>,
    plan: Signal<Option<RenamePlan>>,
    busy: Signal<bool>,
    consumed: Signal<bool>,
    changed: bool,
    refreshing: bool,
) -> Element {
    let mut report = use_signal(|| None::<tv_manager_core::health::HealthReport>);
    let mut scanning = use_signal(|| false);
    let mut scan_error = use_signal(String::new);
    let pending_scan =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<dioxus::core::Task>)));
    use_effect(move || {
        let current = plan();
        let _details = form();
        let c = connection();
        let blocked = busy() || consumed() || changed;
        if let Some(task) = pending_scan.borrow_mut().take() {
            task.cancel();
        }
        report.set(None);
        scan_error.set(String::new());
        scanning.set(false);
        let Some(current) = current.filter(|_| !blocked) else {
            return;
        };
        scanning.set(true);
        *pending_scan.borrow_mut() = Some(spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            match c.health(current).await {
                Ok(value) => report.set(Some(value)),
                Err(error) => scan_error.set(format!("{error:#}")),
            }
            scanning.set(false);
        }));
    });
    rsx! {section {class:"library-tools health-dashboard","aria-busy":(scanning() || busy() || refreshing).to_string(),
        if scanning() || busy() || refreshing {crate::workspace::PageSpinner {label:if refreshing {"Updating library…"}else if scanning(){"Scanning library health…"}else{"Working…"}}}
        div {class:"health-card",
            h3 {"Preview health"}
            p {class:"hint","Automatically scans the current preview. Exact duplicate checks compare file contents and can take time on large libraries."}
            if scanning() {p {class:"hint","Scanning library health…"}}
            else if changed {p {class:"hint",role:"status","Waiting for the updated preview…"}}
            else if consumed() {p {class:"hint","This preview has been applied. Update Workspace details to create a fresh preview."}}
            else if plan.read().is_none() {p {class:"hint","Enter your details in Workspace to generate a preview. Health results will appear here automatically."}}
            if !scan_error().is_empty() {p {class:"error",role:"alert","{scan_error}"}}
            if let Some(value)=report(){HealthCharts {report:value}}
        if let Some(value)=report(){
            if !value.duplicates.is_empty() || !value.unmatched.is_empty() || !value.missing_posters.is_empty() {
                div {class:"health-findings",
                    h4 {"Scan findings"}
                    for group in value.duplicates {details {summary {"Duplicate files"}for path in group {p {"{path}"}}}}
                    for path in value.unmatched {p {class:"hint","Unmatched: {path}"}}
                    for title in value.missing_posters {p {class:"hint","Missing artwork: {title}"}}
                }
            }
        }
        }
    }}
}

#[component]
pub fn RenameHistory(
    connection: Signal<Connection>,
    form: Signal<Form>,
    mut plan: Signal<Option<RenamePlan>>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
    mut consumed: Signal<bool>,
) -> Element {
    let mut history = use_signal(Vec::<tv_manager_core::history::HistoryRecord>::new);
    let mut pending_undo = use_signal(|| None::<uuid::Uuid>);
    let mut history_connection = use_signal(Connection::default);
    let mut history_folder = use_signal(String::new);
    rsx! {div {class:"rename-history-settings","aria-busy":busy().to_string(),
        if busy(){crate::workspace::PageSpinner {label:"Working…"}}
        section {class:"health-card history-panel",
            div {class:"health-card-heading",h3 {"Rename history"}
            button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|{let c=connection();let folder=form.read().folder.clone();history_connection.set(c.clone());history_folder.set(folder.clone());busy.set(true);spawn(async move{match c.history(&folder).await{Ok(items)=>history.set(items),Err(e)=>message.set(e.to_string())}busy.set(false);});},"Load rename history"}
        }
        if !history.read().is_empty(){p {class:"hint",{format!("History for {} / {}",if history_connection.read().remote {history_connection.read().url.clone()}else{history_connection.read().root.clone()},history_folder())}}}
        for item in history(){div {class:"history-item",key:"{item.id}",
            span {"{item.title} · {item.steps.len()} operations · {item.remaining()} to undo"}
            button {r#type:"button",class:"secondary",disabled:busy()||item.remaining()==0,onclick:move |_|pending_undo.set(Some(item.id)),"Undo…"}
        }}
        if let Some(id)=pending_undo(){div {class:"undo-confirm",role:"group","aria-label":"Confirm undo",
            p {"Restore the original file and folder names? Existing files are kept. Files changed since renaming will block Undo. Downloaded artwork is kept."}
            div {class:"pair",
                button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|pending_undo.set(None),"Cancel"}
                button {r#type:"button",class:"primary",disabled:busy(),onclick:move |_|{let c=history_connection();let folder=history_folder();busy.set(true);pending_undo.set(None);spawn(async move{match c.undo(&folder,id).await{Ok(_)=>{plan.set(None);consumed.set(false);message.set("Original names restored. The preview will refresh automatically.".into());},Err(e)=>message.set(format!("Undo stopped: {e:#}. Reload history before retrying."))}if let Ok(items)=c.history(&folder).await{history.set(items);}busy.set(false);});},"Restore original names"}
            }
        }}
    }}}
}

#[component]
fn HealthCharts(report: tv_manager_core::health::HealthReport) -> Element {
    let unmatched = report.unmatched.len();
    let matched = report.files.saturating_sub(unmatched + report.conflicts);
    let total = report.files.max(1) as f64;
    let matched_end = matched as f64 / total * 100.0;
    let conflict_end = (matched + report.conflicts) as f64 / total * 100.0;
    let ring = if report.files == 0 {
        "background:#303030".to_string()
    } else {
        format!("background:conic-gradient(#a9d8bf 0% {matched_end}%,#ee997a {matched_end}% {conflict_end}%,#e6c67a {conflict_end}% 100%)")
    };
    let issues = [
        ("Missing episodes", report.missing_episodes),
        ("Unmatched videos", unmatched),
        ("Filename conflicts", report.conflicts),
        ("Missing posters", report.missing_posters.len()),
        ("Exact duplicate groups", report.duplicates.len()),
    ];
    let max = issues
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0)
        .max(1);
    rsx! {div {class:"health-charts",
        section {class:"health-chart","aria-label":"Video matching chart",
            h4 {"Video matching"}
            div {class:"health-coverage",
                div {class:"health-ring",style:"{ring}",role:"img","aria-label":"{matched} matched videos, {report.conflicts} conflicts, {unmatched} unmatched videos",
                    div {strong {"{report.files}"} span {"videos scanned"}}
                }
                ul {class:"health-legend",
                    li {span {class:"health-dot matched"} "Matched" strong {"{matched}"}}
                    li {span {class:"health-dot conflict"} "Conflicts" strong {"{report.conflicts}"}}
                    li {span {class:"health-dot unmatched"} "Unmatched" strong {"{unmatched}"}}
                }
            }
        }
        section {class:"health-chart","aria-label":"Library issues chart",
            h4 {"Items to review"}
            p {class:"hint","Counts by category; a video can have more than one issue."}
            for (label,count) in issues {
                div {class:"health-bar-row",
                    div {span {"{label}"} strong {"{count}"}}
                    div {class:"health-bar-track",role:"img","aria-label":"{label}: {count}",span {style:format!("width:{}%",count as f64 / max as f64 * 100.0)}}
                }
            }
        }
    }}
}

#[component]
pub fn ArtworkPicker(
    form: Signal<Form>,
    connection: Signal<Connection>,
    plan: Signal<Option<RenamePlan>>,
    mut inventory: Signal<Vec<tv_manager_core::artwork::Poster>>,
    mut selection: Signal<Option<crate::poster::Selection>>,
    stale: bool,
    active: Signal<&'static str>,
) -> Element {
    let mut search = use_signal(String::new);
    let sort_order = use_signal(|| "season-asc".to_string());
    let mut show_sort = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(String::new);
    let pending =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<dioxus::core::Task>)));
    use_effect(move || {
        let current = plan();
        let c = connection();
        let _details = form();
        let visible = active() == "posters";
        if let Some(task) = pending.borrow_mut().take() {
            task.cancel();
        }
        inventory.set(Vec::new());
        selection.set(None);
        error.set(String::new());
        loading.set(false);
        if let Some(p) = current.filter(|_| !stale && visible) {
            loading.set(true);
            *pending.borrow_mut() = Some(spawn(async move {
                match c.artwork(p).await {
                    Ok(items) => {
                        inventory.set(items);
                    }
                    Err(e) => error.set(format!("{e:#}")),
                }
                loading.set(false);
            }));
        }
    });
    let current = plan();
    let mut art = current
        .as_ref()
        .filter(|_| !stale)
        .map(|p| p.show.artwork.clone())
        .unwrap_or_default();
    let query = search().trim().to_lowercase();
    let show_name = current
        .as_ref()
        .map(|p| p.show.name.to_lowercase())
        .unwrap_or_default();
    let items = inventory();
    let mut visible: Vec<_> = items
        .iter()
        .filter(|p| {
            query.is_empty() || format!("{} {}", p.title.to_lowercase(), show_name).contains(&query)
        })
        .cloned()
        .collect();
    visible.sort_by(|a, b| {
        crate::poster::compare_posters(
            &a.title,
            a.local.is_some(),
            &b.title,
            b.local.is_some(),
            &sort_order(),
        )
    });
    art.alternatives.retain(|p| {
        query.is_empty() || format!("{} {}", p.label.to_lowercase(), show_name).contains(&query)
    });
    art.alternatives.sort_by(|a, b| {
        crate::poster::compare_posters(&a.label, false, &b.label, false, &sort_order())
    });
    let selected = selection();
    rsx! {section {class:"artwork-browser","aria-label":"Search available artwork",
        div {class:"library-search-tools artwork-search-tools",
        div {class:"library-search artwork-search",crate::workspace::Icon {name:"search"}
            input {r#type:"search",value:search(),placeholder:"Search artwork by title or season…","aria-label":"Search available posters",oninput:move |e|search.set(e.value())}
        }
            button {class:"library-ordering-button",r#type:"button",title:"Sort posters","aria-label":"Sort posters","aria-haspopup":"dialog","aria-controls":"artwork-sort-dialog",onclick:move |_|show_sort.set(true),crate::workspace::Icon {name:"ordering"}}
        }
        if show_sort(){crate::poster::SortPopup {order:sort_order,onclose:move |_|show_sort.set(false)}}
        if loading(){crate::workspace::PageSpinner {label:"Loading posters…"}}
        if !error().is_empty(){p {class:"error",role:"alert","{error}"}}
        p {class:"hint","Local posters appear in color. Grayed-out provider posters indicate missing local artwork."}
        if visible.is_empty() && !loading(){p {class:"artwork-empty","No posters match your search. Enter details in Workspace to load artwork."}}
        div {class:"poster-grid",
            for item in visible {
                button {r#type:"button",key:"{item.id}",class:if selected.as_ref().is_some_and(|s|s.slot.id==item.id && Some(&s.url)==item.local.as_ref().or(item.provider.as_ref())){"poster-grid-item selected"}else{"poster-grid-item"},
                    disabled:item.local.is_none() && item.provider.is_none(),
                    "aria-label":format!("Preview {}",item.title),"aria-pressed":selected.as_ref().is_some_and(|s|s.slot.id==item.id && Some(&s.url)==item.local.as_ref().or(item.provider.as_ref())).to_string(),
                    onclick:{let item=item.clone();move |_|{if let Some(url)=item.local.clone().or(item.provider.clone()){selection.set(Some(crate::poster::Selection {slot:item.clone(),url}));}}},
                    div {class:if item.local.is_none(){"provider-poster"}else{"local-poster"},crate::poster::PosterCard {key:"{item.fingerprint:?}-{item.provider:?}",url:item.local.clone().or(item.provider.clone()),title:item.title.clone(),compact:false}}
                    if item.local.is_none() && item.provider.is_some() {
                        span {class:"poster-download-badge",title:"Available to download", "aria-label":"Available to download",
                            svg {view_box:"0 0 24 24",width:"17",height:"17",fill:"none",stroke:"currentColor",stroke_width:"1.8",stroke_linecap:"round",stroke_linejoin:"round",
                                path {d:"M12 3v12m-5-5 5 5 5-5M5 16v5h14v-5"}
                            }
                        }
                    }
                    span {class:"poster-source",if item.local.is_some(){"On device"}else{"Missing locally"}}
                }
            }
        }
        if !art.alternatives.is_empty() && !items.is_empty() {
            h3 {class:"alternate-art-heading","Provider alternatives"}
            hr {class:"provider-alternatives-divider"}
            div {class:"poster-grid",
                for choice in art.alternatives {button {r#type:"button",class:if selected.as_ref().is_some_and(|s|s.url==choice.url){"poster-grid-item selected"}else{"poster-grid-item"},key:"{choice.url}",onclick:{let url=choice.url.clone();move |_|{let slot=selection.peek().as_ref().map(|s|s.slot.clone()).or_else(||inventory.peek().first().cloned());if let Some(slot)=slot{selection.set(Some(crate::poster::Selection {slot,url:url.clone()}));}}},
                    crate::poster::PosterCard {url:Some(choice.url.clone()),title:choice.label.clone(),compact:false}
                }}
            }
        }
        for warning in art.warnings {p {class:"hint","{warning}"}}
        if !form.read().options.tmdb_key.is_empty() || art.poster.as_ref().is_some_and(|url|url.starts_with("https://image.tmdb.org/")){a {href:"https://www.themoviedb.org",target:"_blank",rel:"noopener noreferrer","Artwork via TMDB"}p {class:"hint","This product uses the TMDB API but is not endorsed or certified by TMDB."}}
    }}
}

#[component]
pub fn NumberingOptions(mut form: Signal<Form>, message: Signal<String>) -> Element {
    let value = form.read().clone();
    rsx! {
        if !value.movie {
            label {"Episode numbering" select {value:match value.options.numbering {tv_manager_core::Numbering::Sequence=>"sequence",tv_manager_core::Numbering::Detected=>"detected",tv_manager_core::Numbering::Absolute=>"absolute"},onchange:move|e|form.write().options.numbering=match e.value().as_str(){"detected"=>tv_manager_core::Numbering::Detected,"absolute"=>tv_manager_core::Numbering::Absolute,_=>tv_manager_core::Numbering::Sequence},
                option {value:"sequence","Sequential files in each season"}option {value:"detected","Read SxxExx from filenames"}option {value:"absolute","Anime absolute episode numbers"}
            }}
            if value.options.numbering==tv_manager_core::Numbering::Absolute {
                label {"First absolute episode number" input {r#type:"number",min:"1",value:"{value.options.absolute_start.max(1)}",oninput:move|e|{if let Ok(value)=e.value().parse::<u32>(){form.write().options.absolute_start=value.max(1);}}}}
                p {class:"hint","Maps absolute numbers to regular episodes in the provider's season order. S00Exx files remain specials. Confirm the episode order before renaming."}
            }
            details {class:"feature-block",summary {"Custom episode order / specials mapping"}
                label {"One relative path | season | episode per line" textarea {rows:"4",placeholder:"Season 1/episode.mkv|1|3",value:"{value.options.episode_map}",oninput:move|e|form.write().options.episode_map=e.value()}}
                p {class:"hint","Explicit mappings override detected and absolute numbering. Use season 0 for specials."}
            }
        }
        Profiles {form,message}
        label {class:"check",input {r#type:"checkbox",checked:value.options.companions,onchange:move|e|form.write().options.companions=e.checked()},"Rename subtitles, external audio and NFO files with videos"}
    }
}

#[component]
pub fn OrderingPopup(
    mut form: Signal<Form>,
    message: Signal<String>,
    disabled: bool,
    onclose: EventHandler<()>,
) -> Element {
    let form_value = form();
    use_effect(move || {
        let _ = document::eval("document.getElementById('ordering-dialog')?.showModal();");
    });
    rsx! {dialog {id:"ordering-dialog",class:"panel controls ordering-dialog","aria-labelledby":"ordering-dialog-title",oncancel:move |_|onclose.call(()),
        button {class:"ordering-close",r#type:"button",autofocus:true,title:"Close ordering settings","aria-label":"Close ordering settings",onclick:move |_|onclose.call(()),crate::workspace::Icon {name:"close"}}
        fieldset {disabled,
                        section { class: "settings-group ordering-group",
                            h2 { id:"ordering-dialog-title","Ordering" }
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
                            NumberingOptions {form,message}
                        }

        }
        p {class:"hint",role:"status","{message}"}
        button {class:"primary",r#type:"button",onclick:move |_|onclose.call(()),"Done"}
    }}
}
