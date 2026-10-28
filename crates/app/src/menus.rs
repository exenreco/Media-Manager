use crate::{workspace::Icon, Form};
use dioxus::prelude::*;

pub fn close(id: &str) {
    let id = serde_json::to_string(id).expect("dialog id");
    let _ = document::eval(&format!(
        "window.mediaManagerMenuSheets?.get({id})?.dismiss()"
    ));
}

#[component]
pub fn SheetDialog(id: String, title: String, onclose: EventHandler, children: Element) -> Element {
    let effect_id = id.clone();
    use_effect(move || {
        let mut bridge = document::eval(include_str!("menu-sheet.js"));
        let id = effect_id.clone();
        spawn(async move {
            if bridge.send(id).is_err() {
                return;
            }
            while let Ok(action) = bridge.recv::<String>().await {
                if action == "dismiss" {
                    onclose.call(());
                }
            }
        });
    });
    let cleanup_id = id.clone();
    use_drop(move || {
        let id = serde_json::to_string(&cleanup_id).expect("dialog id");
        let _ = document::eval(&format!(
            "window.mediaManagerMenuSheets?.get({id})?.cleanup()"
        ));
    });
    let heading = format!("{id}-title");
    rsx! {dialog {id:id.clone(),class:"panel controls ordering-dialog menu-sheet sheet-surface","aria-labelledby":heading.clone(),
        div {class:"menu-sheet-chrome",
            div {class:"preview-sheet-grip","aria-label":"Drag down to close",span {"aria-hidden":"true"}}
            div {class:"menu-sheet-toolbar",
                button {class:"menu-sheet-back",r#type:"button","aria-label":"Back",onclick:{let id=id.clone();move |_|close(&id)},Icon {name:"back"}}
                button {class:"menu-sheet-close",r#type:"button",autofocus:true,"aria-label":"Close settings",onclick:{let id=id.clone();move |_|close(&id)},Icon {name:"close"}}
            }
        }
        div {class:"menu-sheet-content",h2 {id:heading,"{title}"}{children}}
        footer {class:"menu-sheet-footer",button {class:"primary",r#type:"button",onclick:move |_|close(&id),"Done"}}
    }}
}

#[component]
pub fn ProviderPicker(mut form: Signal<Form>) -> Element {
    let mut showing = use_signal(|| false);
    let movie = form.read().movie;
    let options = if movie {
        vec![
            ("manual", "Entered title and year"),
            ("tmdb", "TMDB"),
            ("omdb", "OMDb"),
            ("custom", "Custom movie API (TMDB-compatible)"),
        ]
    } else {
        vec![
            ("tvmaze", "TVMaze"),
            ("omdb", "OMDb"),
            ("custom", "Custom TVMaze-compatible API"),
            ("manual", "Manual titles"),
            ("batch", "Batch sequence"),
            ("web", "Episode guide URL"),
        ]
    };
    let value = form.read().source.clone();
    let current = options
        .iter()
        .find(|(id, _)| *id == value)
        .copied()
        .unwrap_or(options[0]);
    let title = if movie {
        "Movie source"
    } else {
        "Episode source"
    };
    rsx! {div {class:"provider-picker",
        label {id:"provider-picker-label",r#for:"provider-picker-select","{title}"}
        select {id:"provider-picker-select",class:"desktop-provider-select",value:current.0,onchange:move |event|set_source(form,event.value()),
            for (id,label) in options.iter().copied() {option {value:id,"{label}"}}
        }
        button {class:"mobile-provider-trigger",r#type:"button","aria-haspopup":"dialog","aria-controls":"provider-dialog","aria-expanded":showing().to_string(),"aria-labelledby":"provider-picker-label provider-picker-value",onclick:move |_|showing.set(true),
            span {id:"provider-picker-value","{current.1}"}Icon {name:"chevron-down"}
        }
        if showing() {SheetDialog {id:"provider-dialog",title,onclose:move |_|showing.set(false),
            div {class:"provider-options",role:"radiogroup","aria-label":title,
                for (id,label) in options {button {class:if current.0==id {"provider-option selected"}else{"provider-option"},r#type:"button",role:"radio","aria-checked":(current.0==id).to_string(),onclick:move |_|{set_source(form,id.into());close("provider-dialog");},
                    span {"{label}"}if current.0==id {Icon {name:"check"}}
                }}
            }
        }}
    }}
}

fn set_source(mut form: Signal<Form>, source: String) {
    let mut current = form.write();
    current.source = source;
    current.options.selected_id.clear();
    current.options.poster_override = None;
    current.options.season_overrides.clear();
}
