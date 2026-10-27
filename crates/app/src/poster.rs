use dioxus::prelude::*;

const PLACEHOLDER: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='180' height='270' viewBox='0 0 180 270'%3E%3Crect width='180' height='270' rx='12' fill='%230e1625'/%3E%3Crect x='48' y='76' width='84' height='94' rx='8' fill='%2329364d' stroke='%23546a86' stroke-width='2'/%3E%3Cpath d='M76 101L109 123L76 145Z' fill='%239de8d0'/%3E%3Cpath d='M57 88H65M57 106H65M57 124H65M57 142H65M57 160H65M115 88H123M115 106H123M115 124H123M115 142H123M115 160H123' stroke='%2393a5bf' stroke-width='4'/%3E%3Ctext x='90' y='201' text-anchor='middle' fill='%2393a5bf' font-family='sans-serif' font-size='11' letter-spacing='2'%3ENO POSTER%3C/text%3E%3C/svg%3E";

#[derive(Clone, PartialEq)]
pub struct Selection {
    pub slot: tv_manager_core::artwork::Poster,
    pub url: String,
}

#[component]
pub fn PosterInspector(
    mut selection: Signal<Option<Selection>>,
    mut inventory: Signal<Vec<tv_manager_core::artwork::Poster>>,
    plan: Signal<Option<tv_manager_core::rename::RenamePlan>>,
    connection: Signal<crate::backend::Connection>,
    mut busy: Signal<bool>,
) -> Element {
    let mut replace = use_signal(|| false);
    let mut status = use_signal(String::new);
    use_effect(move || {
        let _ = selection();
        replace.set(false);
        status.set(String::new());
    });
    let Some(chosen) = selection() else {
        return rsx! {};
    };
    let local = chosen.slot.local.clone();
    let download = chosen
        .slot
        .provider
        .as_ref()
        .filter(|_| chosen.url.starts_with("data:"))
        .cloned()
        .unwrap_or(chosen.url.clone());
    let can_save =
        download.starts_with("https://") && (chosen.slot.local_path.is_none() || replace());
    rsx! {section {class:"poster-inspector",
        div {class:"inspector-heading",h2 {"Poster preview"}button {class:"quiet-icon",title:"Clear poster selection","aria-label":"Clear poster selection",onclick:move |_|selection.set(None),crate::workspace::Icon {name:"close"}}}
        h3 {"{chosen.slot.title}"}
        label {"Preview alternate artwork for" select {value:chosen.slot.id.clone(),onchange:move |e|{
            let item=inventory.peek().iter().find(|p|p.id==e.value()).cloned();
            if let (Some(slot),Some(current))=(item,selection()) {selection.set(Some(Selection {slot,url:current.url}));}
        },for item in inventory() {option {value:"{item.id}","{item.title}"}}}}

        div {class:if local.is_some(){"poster-comparison"}else{"poster-comparison single"},
            if let Some(existing)=local.clone() {figure {img {src:existing,alt:"Existing local poster"}figcaption {"Existing poster"}}}
            figure {img {src:"{download}",alt:"Selected poster"}figcaption {"Selected poster"}}
        }
        if chosen.slot.local_path.is_some() {label {class:"check",input {r#type:"checkbox",checked:replace(),onchange:move |e|replace.set(e.checked())}"Replace existing poster"}}
        button {class:"primary",disabled:busy() || !can_save || plan.read().is_none(),onclick:{let slot=chosen.slot.clone();let url=download.clone();move |_|{
            let Some(p)=plan() else{return;};let c=connection();
            let save=tv_manager_core::artwork::Save {slot:slot.id.clone(),url:url.clone(),replace:replace(),expected:slot.fingerprint.clone()};
            busy.set(true);status.set("Saving poster…".into());
            let slot=slot.clone();let url=url.clone();
            spawn(async move {match c.save_artwork(p,save).await {
                Ok(items)=>{let updated=items.iter().find(|p|p.id==slot.id).cloned();inventory.set(items);if let Some(slot)=updated {selection.set(Some(Selection {slot,url}));}status.set("Poster saved.".into());},
                Err(error)=>status.set(format!("{error:#}")),
            }busy.set(false);});
        }},if busy(){"Saving…"}else{"Save selected poster"}}
        p {class:"hint",role:"status","{status}"}
    }}
}

#[component]
pub fn PosterCard(url: Option<String>, title: String, compact: bool) -> Element {
    let mut failed = use_signal(|| false);
    let source = if failed() {
        PLACEHOLDER
    } else {
        url.as_deref().unwrap_or(PLACEHOLDER)
    };
    rsx! {
        figure { class: if compact { "poster-card compact" } else { "poster-card" },
            img { src: source, alt: "{title}", loading: "lazy", referrerpolicy: "no-referrer",
                onerror: move |_| failed.set(true) }
            figcaption {
                "{title}"
                if url.is_none() { span { class: "hint", " — No poster available" } }
                else if failed() { span { class: "hint", " — Poster could not load" } }
            }
        }
    }
}

pub fn compare_posters(
    a_title: &str,
    a_local: bool,
    b_title: &str,
    b_local: bool,
    order: &str,
) -> std::cmp::Ordering {
    let title = a_title.to_lowercase().cmp(&b_title.to_lowercase());
    match order {
        "title-desc" => title.reverse(),
        "local-first" => b_local.cmp(&a_local).then(title),
        "missing-first" => a_local.cmp(&b_local).then(title),
        "season-asc" | "season-desc" => {
            let a = tv_manager_core::rename::season_in_name(a_title);
            let b = tv_manager_core::rename::season_in_name(b_title);
            a.is_some()
                .cmp(&b.is_some())
                .then_with(|| {
                    if order == "season-desc" {
                        b.cmp(&a)
                    } else {
                        a.cmp(&b)
                    }
                })
                .then(title)
        }
        _ => title,
    }
}

#[component]
pub fn SortPopup(mut order: Signal<String>, onclose: EventHandler<()>) -> Element {
    use_effect(move || {
        let _ = document::eval("document.getElementById('artwork-sort-dialog')?.showModal();");
    });
    rsx! {dialog {id:"artwork-sort-dialog",class:"panel controls ordering-dialog artwork-sort-dialog","aria-labelledby":"artwork-sort-title",oncancel:move |_|onclose.call(()),
        button {class:"ordering-close",r#type:"button",autofocus:true,title:"Close sort options","aria-label":"Close sort options",onclick:move |_|onclose.call(()),crate::workspace::Icon {name:"close"}}
        h2 {id:"artwork-sort-title","Sort posters"}
        div {class:"poster-sort-options",
            for (value,label) in [("season-asc","Season: first to last"),("season-desc","Season: last to first"),("title-asc","Title: A–Z"),("title-desc","Title: Z–A"),("local-first","Local posters first"),("missing-first","Missing posters first")] {
                label {class:"check",input {r#type:"radio",name:"poster-sort",value,checked:order()==value,onchange:move |_|order.set(value.into())}"{label}"}
            }
        }
        button {class:"primary",r#type:"button",onclick:move |_|onclose.call(()),"Done"}
    }}
}
#[cfg(test)]
mod sort_tests {
    use super::*;
    #[test]
    fn seasons_sort_numerically_and_keep_main_poster_first() {
        let mut names = ["Season 10 poster", "Show poster", "Season 02 poster"];
        names.sort_by(|a, b| compare_posters(a, false, b, false, "season-asc"));
        assert_eq!(
            names,
            ["Show poster", "Season 02 poster", "Season 10 poster"]
        );
        names.sort_by(|a, b| compare_posters(a, false, b, false, "season-desc"));
        assert_eq!(
            names,
            ["Show poster", "Season 10 poster", "Season 02 poster"]
        );
    }
    #[test]
    fn availability_sort_places_local_and_missing_images_as_requested() {
        assert!(compare_posters("Z", true, "A", false, "local-first").is_lt());
        assert!(compare_posters("Z", true, "A", false, "missing-first").is_gt());
    }
}
