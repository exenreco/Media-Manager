use dioxus::prelude::*;

// page, target selector, heading, explanation. Only real, visible controls are highlighted.
const STEPS: &[(&str, &str, &str, &str)] = &[
    ("welcome", ".titlebar-media-toggle", "TV shows or movies", "Choose the type of media you want to organize. Click the highlighted toggle to continue, or use Next."),
    ("welcome", ".connection-tabs", "Choose a connection", "Local uses a folder on this device. Remote connects to your desktop library. Select this control to continue the tour; connection settings stay unchanged."),
    ("welcome", "input[placeholder='Select a folder or enter its absolute path']", "Your media folder", "This field holds your local library location. In normal use, enter a folder path here. The guide does not change your library."),
    ("welcome", "[data-tour=browse-folder]", "Browse for a folder", "Use Browse folders to choose your media folder. During the guide, clicking this highlighted button continues without opening the picker."),
    ("welcome", ".mobile-provider-trigger, .api-group select", "Metadata provider", "Choose where show or movie details come from. Some providers require an API key. Clicking here continues without changing your provider."),
    ("welcome", ".welcome-continue", "Open your workspace", "After connecting your library, this button opens Details. Click it now to visit the Details page in the guide."),
    ("naming", ".desktop-rail [data-tour=naming], .dock-bar [data-tour=naming]", "Details navigation", "Use this pencil icon to open Details and configure your titles and naming options."),
    ("naming", "#library-settings input[placeholder='Enter a title']", "Find a title", "Enter the show or movie title here, then review the provider match. The tour leaves your existing title unchanged."),
    ("naming", ".naming-group button", "Match a title", "Find matching titles searches your chosen provider. Review the results before selecting a match. During the guide, clicking this button continues without sending a search."),
    ("naming", "#library-settings input[type=number]", "Year and season", "Use the year to distinguish remakes. For TV shows, the season field can narrow the files you want to organize."),
    ("preview", ".desktop-rail [data-tour=preview], .dock-bar [data-tour=preview]", "Library navigation", "Use this library icon to return to your files from any page."),
    ("preview", ".library-tabs button[aria-controls='rename-preview']", "Your Library", "The Library tab lists media files and proposed filenames. Review matches and conflicts before renaming anything."),
    ("preview", "input[aria-label='Search library files']", "Search your library", "This search field filters files in your library. Click the highlighted field to continue."),
    ("preview", "button[aria-label='Open ordering settings']", "Episode ordering", "This icon opens ordering and naming options. During the guide, clicking it continues without changing your settings."),
    ("preview", ".row-action[aria-label='Preview media file']", "Preview a file", "This icon opens the selected file in the media inspector. Supported media can be previewed there. Clicking it in the guide leaves your file selection unchanged."),
    ("preview", ".apply-actions .primary", "Apply reviewed renames", "This button applies the reviewed plan after confirmation. The guide never renames files: clicking the highlighted button only continues the tour."),
    ("posters", ".library-tabs button[aria-controls='poster-naming-settings']", "Artwork", "The Artwork tab lets you browse images for your media. We have opened the actual Artwork page for this part of the guide."),
    ("posters", "input[aria-label='Search available posters']", "Search artwork", "Filter artwork by title or season with this field. Your artwork selection stays unchanged during the tour."),
    ("posters", "button[aria-label='Sort posters']", "Sort artwork", "This icon opens the artwork sorting options. Click the highlighted icon to continue without changing the sort."),
    ("posters", ".poster-grid-item", "Inspect artwork", "Click a poster in normal use to inspect its image and available actions. The guide does not select, download, or replace artwork."),
    ("inspector", ".preview-sheet-close, .media-inspector .quiet-icon", "Close the media inspector", "This icon closes the selected file preview and returns to your library. The guide leaves your selection unchanged."),
    ("health", ".desktop-rail [data-tour=health], .dock-more", "Library health", "The Health icon opens library checks. On mobile, find Health under the highlighted More tools icon. Health normally scans your current preview automatically; scanning is paused during this guide."),
    ("settings", ".desktop-rail [data-tour=settings], .dock-more", "Settings navigation", "The gear opens Settings. On mobile, open the highlighted More tools icon to find Settings."),
    ("settings", ".rename-history-settings button", "Rename history", "Load rename history here to review previous operations and available Undo actions. The guide does not load or restore files."),
    ("settings", ".pro-actions button, .billing-buy", "One-time upgrade", "This is the purchase button for the upgrade available on your platform. Clicking it during the guide never starts a purchase."),
    ("settings", ".pro-actions button:nth-child(2), .billing-restore", "Restore a purchase", "Use Restore to check the current store account for an existing purchase. The guide does not contact the store or change ownership."),
    ("settings", ".billing-actions ~ button, .ad-rewarded", "Ad preferences", "On mobile, this button offers an ad for a temporary ad-free period. Clicking it in the guide does not show an ad."),
    ("settings", ".walkthrough-replay", "Replay from any page", "Use this question mark anytime to restart the interactive guide at your current page. Finish returns you to the page where you started."),
];

pub fn page_index(page: &str) -> usize {
    STEPS.iter().position(|s| s.0 == page).unwrap_or(0)
}

fn marker() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::session::path()?.with_file_name("walkthrough-v1"))
}

fn completed() -> bool {
    marker()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .is_some_and(|v| v == b"completed\n")
}

fn save_completion() -> anyhow::Result<()> {
    let path = marker()?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Settings directory unavailable"))?;
    std::fs::create_dir_all(parent)?;
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(b"completed\n")?;
    file.persist(path)?;
    Ok(())
}

#[component]
pub fn Walkthrough(
    mut tour: Signal<Option<usize>>,
    ready: bool,
    mut active: Signal<&'static str>,
) -> Element {
    let mut checked = use_signal(|| false);
    let mut origin = use_signal(|| None::<&'static str>);
    use_effect(move || match tour() {
        Some(index) => {
            if origin.peek().is_none() {
                origin.set(Some(*active.peek()));
            }
            active.set(STEPS[index.min(STEPS.len() - 1)].0);
        }
        None => {
            if let Some(page) = origin.take() {
                active.set(page);
            }
        }
    });
    let mut error = use_signal(String::new);
    use_effect(use_reactive!(|(ready,)| {
        if ready && !checked() {
            checked.set(true);
            if !completed() && tour.peek().is_none() {
                tour.set(Some(0));
            }
        }
    }));
    let close = move |_| {
        if let Err(e) = save_completion() {
            error.set(format!(
                "Could not save walkthrough preference: {e}. The guide may appear again next time."
            ));
        }
        tour.set(None);
    };
    rsx! {
        if let Some(index) = tour() {
            TourDialog {index:index.min(STEPS.len()-1), tour, onclose:close}
        }
        if !error().is_empty() {p {class:"walkthrough-error",role:"alert", "{error}"}}
    }
}

#[component]
fn TourDialog(index: usize, mut tour: Signal<Option<usize>>, onclose: EventHandler<()>) -> Element {
    use_drop(|| {
        let _ = document::eval("window.mediaManagerTour?.cleanup();");
    });
    let mut previous = use_signal(|| index);
    use_effect(use_reactive!(|(index,)| {
        let backwards = index < *previous.peek();
        previous.set(index);
        let config = serde_json::json!({"selector": STEPS[index].1});
        let script = format!("const config={config};\n{}", include_str!("walkthrough.js"));
        spawn(async move {
            let mut eval = document::eval(&script);
            if let Ok(action) = eval.recv::<String>().await {
                if *tour.peek() == Some(index) {
                    match action.as_str() {
                        "next" if index + 1 < STEPS.len() => tour.set(Some(index + 1)),
                        "next" => onclose.call(()),
                        "skip" => tour.set(Some(if backwards {
                            index.saturating_sub(1)
                        } else {
                            (index + 1).min(STEPS.len() - 1)
                        })),
                        "close" => onclose.call(()),
                        _ => (),
                    }
                }
            }
        });
    }));
    let (_, _, heading, description) = STEPS[index];
    rsx! {
        div {id:"app-walkthrough",class:"app-walkthrough",role:"dialog","aria-modal":"false","aria-labelledby":"walkthrough-heading",
            div {class:"walkthrough-top",span {"INTERACTIVE GUIDE"} button {r#type:"button",class:"secondary",onclick:move |_|onclose.call(()),"Skip guide"}}
            div {class:"walkthrough-body",
                p {class:"walkthrough-count","{index + 1} of {STEPS.len()}"}
                h2 {id:"walkthrough-heading",tabindex:"-1","{heading}"}
                p {id:"walkthrough-description","{description}"}
                p {id:"walkthrough-unavailable",hidden:true,"This control becomes available when your library or platform supports it. Use Next to continue."}
            }
            div {class:"walkthrough-actions",
                button {r#type:"button",class:"secondary",disabled:index==0,onclick:move |_|tour.set(Some(index.saturating_sub(1))),"Back"}
                if index+1==STEPS.len() {button {r#type:"button",class:"primary",onclick:move |_|onclose.call(()),"Finish"}}
                else {button {r#type:"button",class:"primary",onclick:move |_|tour.set(Some(index+1)),"Next"}}
            }
        }
    }
}
