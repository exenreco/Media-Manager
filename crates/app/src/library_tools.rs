use crate::{backend::Connection, pairing, profiles, Form};
use dioxus::prelude::*;
use tv_manager_core::{rename::RenamePlan, ShowMatch};

#[component]
pub fn MatchPicker(
    mut form: Signal<Form>,
    connection: Signal<Connection>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
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
                        matches.set(vec![]);message.set("Match selected. Build the preview to review its episodes.".into());
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
            if let Some(profile)=profiles.read().iter().find(|p|p.name==e.value()){profile.apply(&mut form.write());message.set("Naming profile loaded. Rebuild the preview before renaming.".into());}
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
    mut connection: Signal<Connection>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
) -> Element {
    let mut qr = use_signal(String::new);
    let mut invitation = use_signal(|| None::<pairing::Invitation>);
    let mut text = use_signal(String::new);
    rsx! {details {class:"feature-block",summary {"Pair another device"}
        p {class:"hint","Connect through an HTTPS Media Manager server. Generate a code on a connected device, then import its QR image on your phone."}
        if connection.read().remote {
            button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|{let c=connection();busy.set(true);spawn(async move{
                let result=async {pairing::Invitation::parse(&serde_json::to_string(&pairing::Invitation{version:1,url:c.url.clone(),code:uuid::Uuid::nil().to_string()})?)?;let code=c.pairing_code().await?;let invite=pairing::Invitation{version:1,url:c.url.clone(),code};let data=serde_json::to_string(&invite)?;pairing::Invitation::parse(&data)?;pairing::qr_uri(&data)}.await;
                match result {Ok(uri)=>{qr.set(uri);message.set("Pairing QR is valid for five minutes and one use.".into());},Err(e)=>message.set(format!("{e:#}"))}busy.set(false);
            });},"Generate pairing QR"}
        }
        if !qr().is_empty(){img {class:"pairing-qr",src:qr(),alt:"Single-use Media Manager pairing QR"}p {class:"hint","Expires after five minutes. Share only with the device you want to connect."}}
        button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|{busy.set(true);spawn(async move{match pairing::pick_invitation().await{Ok(value)=>invitation.set(Some(value)),Err(e)=>message.set(e.to_string())}busy.set(false);});},"Import pairing QR image"}
        label {"Or paste pairing invitation" textarea {rows:"2",value:text(),oninput:move|e|text.set(e.value())}}
        button {r#type:"button",class:"secondary",disabled:busy()||text().is_empty(),onclick:move |_|match pairing::Invitation::parse(&text()){Ok(value)=>invitation.set(Some(value)),Err(e)=>message.set(e.to_string())},"Review invitation"}
        if let Some(value)=invitation(){
            p {"Connect to {value.url}?"}
            button {r#type:"button",class:"primary",disabled:busy(),onclick:move |_|{let value=value.clone();busy.set(true);spawn(async move{match Connection::redeem_pairing(value.url,value.code).await{Ok(c)=>{connection.set(c);invitation.set(None);message.set("Device paired. Choose the server subfolder and build a preview.".into());},Err(e)=>message.set(e.to_string())}busy.set(false);});},"Confirm pairing"}
        }
    }}
}

#[component]
pub fn HistoryHealth(
    connection: Signal<Connection>,
    form: Signal<Form>,
    mut plan: Signal<Option<RenamePlan>>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
    mut consumed: Signal<bool>,
    changed: bool,
) -> Element {
    let mut history = use_signal(Vec::<tv_manager_core::history::HistoryRecord>::new);
    let mut report = use_signal(|| None::<tv_manager_core::health::HealthReport>);
    use_effect(move || {
        let _current = plan.read().as_ref().map(|p| p.id);
        report.set(None);
    });
    let mut pending_undo = use_signal(|| None::<uuid::Uuid>);
    let mut history_connection = use_signal(Connection::default);
    let mut history_folder = use_signal(String::new);
    rsx! {details {class:"feature-block library-tools",summary {"Library health & rename history"}
        div {class:"pair",
            button {r#type:"button",class:"secondary",disabled:busy()||plan.read().is_none()||consumed()||changed,onclick:move |_|{let Some(p)=plan()else{return;};let c=connection();busy.set(true);report.set(None);spawn(async move{match c.health(p).await{Ok(value)=>report.set(Some(value)),Err(e)=>message.set(e.to_string())}busy.set(false);});},"Scan preview health"}
            button {r#type:"button",class:"secondary",disabled:busy(),onclick:move |_|{let c=connection();let folder=form.read().folder.clone();history_connection.set(c.clone());history_folder.set(folder.clone());busy.set(true);spawn(async move{match c.history(&folder).await{Ok(items)=>history.set(items),Err(e)=>message.set(e.to_string())}busy.set(false);});},"Load rename history"}
        }
        if let Some(value)=report(){
            p {"{value.files} videos · {value.missing_episodes} missing episodes · {value.conflicts} conflicts"}
            p {"{value.unmatched.len()} unmatched · {value.missing_posters.len()} missing posters · {value.duplicates.len()} exact duplicate groups"}
            p {class:"hint","This report covers the current preview. Duplicate checks compare file contents and can take time on large libraries."}
            for group in value.duplicates {details {summary {"Duplicate files"}for path in group {p {"{path}"}}}}
            for path in value.unmatched {p {class:"hint","Unmatched: {path}"}}
            for title in value.missing_posters {p {class:"hint","Missing artwork: {title}"}}
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
                button {r#type:"button",class:"primary",disabled:busy(),onclick:move |_|{let c=history_connection();let folder=history_folder();busy.set(true);pending_undo.set(None);spawn(async move{match c.undo(&folder,id).await{Ok(_)=>{plan.set(None);consumed.set(false);report.set(None);message.set("Original names restored. Build a fresh preview before continuing.".into());},Err(e)=>message.set(format!("Undo stopped: {e:#}. Reload history before retrying."))}if let Ok(items)=c.history(&folder).await{history.set(items);}busy.set(false);});},"Restore original names"}
            }
        }}
    }}
}

#[component]
pub fn ArtworkPicker(
    mut form: Signal<Form>,
    connection: Signal<Connection>,
    mut plan: Signal<Option<RenamePlan>>,
    mut snapshot: Signal<Option<(Form, Connection)>>,
    mut busy: Signal<bool>,
    mut message: Signal<String>,
) -> Element {
    let mut target = use_signal(|| "main".to_string());
    let Some(current) = plan() else {
        return rsx! {};
    };
    let choices = current.show.artwork.alternatives.clone();
    let seasons: std::collections::BTreeSet<_> =
        current.show.episodes.iter().map(|e| e.season).collect();
    rsx! {details {class:"feature-block",summary {"Choose alternate artwork"}
        if choices.is_empty(){p {class:"hint","No alternate artwork returned. Add a TMDB API key in API settings and rebuild the preview to search a second source."}}
        label {"Apply poster to" select {value:target(),onchange:move|e|target.set(e.value()),option {value:"main","Main poster"}for season in seasons {option {value:"{season}","Season {season:02}"}}}}
        div {class:"artwork-choices",for choice in choices {button {r#type:"button",class:"secondary artwork-choice",key:"{choice.url}",disabled:busy(),onclick:{let selected_url=choice.url.clone();move |_|{
            let c=connection();let mut f=form();
            if target()=="main" {f.options.poster_override=Some(selected_url.clone());}else if let Ok(season)=target().parse::<u32>(){f.options.season_overrides.insert(season,selected_url.clone());}
            let request=match f.request(){Ok(r)=>r,Err(e)=>{message.set(e.to_string());return;}};
            busy.set(true);spawn(async move{match c.preview(&request).await{Ok(p)=>{plan.set(Some(p));snapshot.set(Some((f.clone(),c)));form.set(f);message.set("Artwork selection updated in the preview.".into());},Err(e)=>message.set(e.to_string())}busy.set(false);});
        }},img {src:"{choice.url}",alt:"{choice.label}",loading:"lazy",referrerpolicy:"no-referrer"}span {"{choice.label}"}}}}
        for warning in current.show.artwork.warnings {p {class:"hint","{warning}"}}
        if !form.read().options.tmdb_key.is_empty(){a {href:"https://www.themoviedb.org",target:"_blank",rel:"noopener noreferrer","Artwork via TMDB"}p {class:"hint","This product uses the TMDB API but is not endorsed or certified by TMDB."}}
    }}
}

#[component]
pub fn NumberingOptions(mut form: Signal<Form>) -> Element {
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
        label {class:"check",input {r#type:"checkbox",checked:value.options.companions,onchange:move|e|form.write().options.companions=e.checked()},"Rename subtitles, external audio and NFO files with videos"}
    }
}
