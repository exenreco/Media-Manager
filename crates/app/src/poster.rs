use dioxus::prelude::*;

const PLACEHOLDER: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='180' height='270' viewBox='0 0 180 270'%3E%3Crect width='180' height='270' rx='12' fill='%230e1625'/%3E%3Crect x='48' y='76' width='84' height='94' rx='8' fill='%2329364d' stroke='%23546a86' stroke-width='2'/%3E%3Cpath d='M76 101L109 123L76 145Z' fill='%239de8d0'/%3E%3Cpath d='M57 88H65M57 106H65M57 124H65M57 142H65M57 160H65M115 88H123M115 106H123M115 124H123M115 142H123M115 160H123' stroke='%2393a5bf' stroke-width='4'/%3E%3Ctext x='90' y='201' text-anchor='middle' fill='%2393a5bf' font-family='sans-serif' font-size='11' letter-spacing='2'%3ENO POSTER%3C/text%3E%3C/svg%3E";

#[derive(Clone, PartialEq)]
pub struct PosterItem {
    pub title: String,
    pub url: Option<String>,
}

#[component]
pub fn PosterCarousel(posters: Vec<PosterItem>) -> Element {
    let mut active = use_signal(|| 0usize);
    let current = active().min(posters.len().saturating_sub(1));
    let last = posters.len().saturating_sub(1);
    let position = if posters.is_empty() { 0 } else { current + 1 };
    rsx! {
        div { class: "poster-carousel", role: "region", "aria-label": "Show and season poster carousel",
            div { class: "carousel-navigation",
                button { r#type: "button", class: "icon-button", "aria-label": "Previous poster", title: "Previous poster",
                    disabled: current == 0, onclick: move |_| active.set(current.saturating_sub(1)),
                    svg { view_box: "0 0 24 24", width: "18", height: "18", fill: "none", stroke: "currentColor", stroke_width: "2",
                        path { d: "m15 18-6-6 6-6" }
                    }
                }
                span { class: "carousel-position", "aria-live": "polite", "{position} / {posters.len()}" }
                button { r#type: "button", class: "icon-button", "aria-label": "Next poster", title: "Next poster",
                    disabled: current >= last, onclick: move |_| active.set((current + 1).min(last)),
                    svg { view_box: "0 0 24 24", width: "18", height: "18", fill: "none", stroke: "currentColor", stroke_width: "2",
                        path { d: "m9 6 6 6-6 6" }
                    }
                }
            }
            div { class: "carousel-track",
                for (index, item) in posters.iter().enumerate().filter(|(index, _)| index.abs_diff(current) <= 1) {
                    button { r#type: "button", class: if index == current { "poster-select active" } else { "poster-select" },
                        key: "{item.title}-{item.url:?}", "aria-label": format!("View {} poster", item.title),
                        "aria-pressed": (index == current).to_string(), onclick: move |_| active.set(index),
                        PosterCard { key: "{item.url:?}", url: item.url.clone(), title: item.title.clone(), compact: index != current }
                    }
                }
            }
        }
    }
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
            figcaption { "{title}" }
        }
    }
}
