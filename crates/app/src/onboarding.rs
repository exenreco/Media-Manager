use crate::{backend::Connection, workspace::Icon};
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Eq, Default)]
pub enum Notice {
    #[default]
    Hidden,
    Checking,
    Required(String),
    Connected,
    MatchSelected(String),
}

pub fn requires_library(target: &str) -> bool {
    !matches!(target, "welcome" | "settings")
}

pub fn use_navigation(
    connection: Signal<Connection>,
    mut active: Signal<&'static str>,
    mut busy: Signal<bool>,
) -> (Callback<&'static str>, Signal<Notice>) {
    let mut notice = use_signal(Notice::default);
    let mut validated = use_signal(|| None::<Connection>);
    let navigate = use_callback(move |target: &'static str| {
        if busy() {
            return;
        }
        if !requires_library(target) {
            active.set(target);
            notice.set(Notice::Hidden);
            return;
        }
        let candidate = connection();
        if validated.peek().as_ref() == Some(&candidate) && *active.peek() != "welcome" {
            active.set(target);
            return;
        }
        busy.set(true);
        notice.set(Notice::Checking);
        spawn(async move {
            let result = candidate.validate().await;
            if *connection.peek() != candidate {
                validated.set(None);
                active.set("welcome");
                notice.set(Notice::Required(
                    "Connection settings changed. Check them again to continue.".into(),
                ));
            } else {
                match result {
                    Ok(()) => {
                        validated.set(Some(candidate));
                        active.set(target);
                        notice.set(Notice::Connected);
                    }
                    Err(error) => {
                        validated.set(None);
                        active.set("welcome");
                        notice.set(Notice::Required(format!("{error:#}")));
                    }
                }
            }
            busy.set(false);
        });
    });
    (navigate, notice)
}

#[cfg(test)]
mod tests {
    #[test]
    fn purchase_and_restore_settings_are_available_without_a_library() {
        assert!(!super::requires_library("settings"));
        assert!(!super::requires_library("welcome"));
        for page in ["naming", "preview", "posters", "health"] {
            assert!(super::requires_library(page));
        }
    }
}

#[component]
pub fn WelcomeIntro() -> Element {
    rsx! {div {class:"welcome-intro",
        p {class:"eyebrow","WELCOME TO MEDIA MANAGER"}
        h1 {"Your library." br {} span {"In perfect order."}}
        p {class:"welcome-lead","Start with a folder on this device, or connect to your desktop library. Then make every filename feel at home."}
        div {class:"welcome-steps",
            for (number,icon,title,description) in [
                ("01","folder","Connect your library","Choose Local, or use Remote to connect a server or scan a desktop QR code."),
                ("02","naming","Make it yours","Choose a naming style, episode ordering, and artwork in your workspace."),
                ("03","library","Preview. Then rename.","Review the proposed changes first. Files are renamed only after you confirm."),
            ] {
                div {class:"welcome-step",span {class:"welcome-step-icon",Icon {name:icon}} div {span {class:"welcome-step-number","{number}"} h3 {"{title}"} p {"{description}"}}}
            }
        }
    }}
}

#[component]
pub fn ConnectionIsland(
    mut notice: Signal<Notice>,
    mut active: Signal<&'static str>,
    onlibrary: EventHandler<()>,
) -> Element {
    let mut leaving = use_signal(|| false);
    use_effect(move || {
        let state = notice();
        leaving.set(false);
        if matches!(state, Notice::Required(_) | Notice::MatchSelected(_)) {
            let _ = document::eval("document.getElementById('connection-confirm')?.focus();");
        }
        if state == Notice::Connected {
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if *notice.peek() != Notice::Connected {
                    return;
                }
                leaving.set(true);
                tokio::time::sleep(std::time::Duration::from_millis(220)).await;
                if *notice.peek() == Notice::Connected {
                    notice.set(Notice::Hidden);
                }
            });
        }
    });
    let dismiss = use_callback(move |confirm: bool| {
        if leaving() {
            return;
        }
        let matched = matches!(*notice.peek(), Notice::MatchSelected(_));
        leaving.set(true);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(220)).await;
            notice.set(Notice::Hidden);
            if matched {
                if confirm {
                    onlibrary.call(());
                }
                return;
            }
            active.set("welcome");
            if confirm {
                let _ =
                    document::eval("document.getElementById('welcome-connection-title')?.focus();");
            }
        });
    });
    let state = notice();
    if state == Notice::Hidden {
        return rsx! {};
    }
    let warning = matches!(state, Notice::Required(_));
    let matched = matches!(state, Notice::MatchSelected(_));
    let modal = warning || matched;
    let phase = match state {
        Notice::Checking => "checking",
        Notice::Required(_) => "warning",
        Notice::MatchSelected(_) => "match",
        _ => "connected",
    };
    rsx! {
        if modal {div {class:"island-shield","aria-hidden":"true"}}
        section {class:if leaving(){"connection-island leaving"}else{"connection-island"},"data-state":phase,
            role:if modal {"alertdialog"}else{"status"},"aria-modal":modal.to_string(),"aria-labelledby":"island-title","aria-describedby":"island-detail",
            onkeydown:move|e|{if modal && e.key()==Key::Escape {e.prevent_default();dismiss.call(false);}},
            div {class:"island-summary",
                span {class:"island-orb",if warning {Icon {name:"devices"}}else if state==Notice::Checking {span {class:"island-spinner"}}else {Icon {name:"check"}}}
                strong {id:"island-title",if matched {"Title selected"}else if warning {"Connection required"}else if state==Notice::Checking {"Checking your library…"}else {"Library connected"}}
                if warning {span {class:"island-alert","!"}}
            }
            if let Notice::MatchSelected(title)=&state {
                div {class:"island-expanded",
                    p {id:"island-detail","{title} selected. Open Library to review its files and artwork?"}
                    div {class:"island-actions",
                        button {r#type:"button",class:"island-cancel",disabled:leaving(),onclick:move |_|dismiss.call(false),"Stay here"}
                        button {id:"connection-confirm",r#type:"button",class:"island-confirm",disabled:leaving(),onclick:move |_|dismiss.call(true),"Open Library"}
                    }
                }
            } else if let Notice::Required(error)=state {
                div {class:"island-expanded",
                    p {id:"island-detail","{error} Set up a valid connection before opening your workspace."}
                    div {class:"island-actions",
                        button {r#type:"button",class:"island-cancel",disabled:leaving(),onclick:move|_|dismiss.call(false),"Not now"}
                        button {id:"connection-confirm",r#type:"button",class:"island-confirm",disabled:leaving(),onclick:move|_|dismiss.call(true),"Set up connection"}
                    }
                }
            } else {span {id:"island-detail",class:"sr-only",if state==Notice::Checking {"Verifying library access."}else{"Your workspace is ready."}}}
        }
    }
}
