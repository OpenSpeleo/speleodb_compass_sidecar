use crate::state::APP_MENU_TITLE;
use objc2::{MainThreadMarker, runtime::Sel};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::{NSString, NSUserDefaults};
use tauri::{AppHandle, Runtime};

const DISABLED_AUTOMATIC_MENU_ITEMS: [&str; 3] = [
    "NSDisabledCharacterPaletteMenuItem",
    "NSDisabledDictationMenuItem",
    "NSWritingToolsMenuItemDisabled",
];

const OWNED_MENU_ACTIONS: [&[u8]; 6] = [
    b"fireMenuItemAction:",
    b"cut:",
    b"copy:",
    b"paste:",
    b"selectAll:",
    b"terminate:",
];

pub fn disable_automatic_text_items() {
    let defaults = NSUserDefaults::standardUserDefaults();
    for key in DISABLED_AUTOMATIC_MENU_ITEMS {
        defaults.setBool_forKey(true, &NSString::from_str(key));
    }
}

fn is_owned_menu_action(action: Sel) -> bool {
    OWNED_MENU_ACTIONS.contains(&action.name().to_bytes())
}

fn is_owned_menu_item(item: &NSMenuItem) -> bool {
    item.isSeparatorItem() || item.action().is_some_and(is_owned_menu_action)
}

fn remove_edge_and_duplicate_separators(menu: &NSMenu) -> usize {
    let mut removed = 0;

    while menu
        .itemAtIndex(0)
        .is_some_and(|item| item.isSeparatorItem())
    {
        menu.removeItemAtIndex(0);
        removed += 1;
    }

    while menu.numberOfItems() > 0 {
        let last_index = menu.numberOfItems() - 1;
        let Some(item) = menu.itemAtIndex(last_index) else {
            break;
        };
        if !item.isSeparatorItem() {
            break;
        }
        menu.removeItemAtIndex(last_index);
        removed += 1;
    }

    let mut index = menu.numberOfItems() - 1;
    while index > 0 {
        let current_is_separator = menu
            .itemAtIndex(index)
            .is_some_and(|item| item.isSeparatorItem());
        let previous_is_separator = menu
            .itemAtIndex(index - 1)
            .is_some_and(|item| item.isSeparatorItem());
        if current_is_separator && previous_is_separator {
            menu.removeItemAtIndex(index);
            removed += 1;
        }
        index -= 1;
    }

    removed
}

fn remove_injected_items_from_application_menu() -> usize {
    let mtm = MainThreadMarker::new().expect("menu cleanup must run on the main thread");
    let application = NSApplication::sharedApplication(mtm);
    let Some(main_menu) = application.mainMenu() else {
        return 0;
    };

    for index in 0..main_menu.numberOfItems() {
        let Some(item) = main_menu.itemAtIndex(index) else {
            continue;
        };
        if item.title().to_string() != APP_MENU_TITLE {
            continue;
        }
        let Some(submenu) = item.submenu() else {
            return 0;
        };

        let mut removed = 0;
        for submenu_index in (0..submenu.numberOfItems()).rev() {
            let Some(submenu_item) = submenu.itemAtIndex(submenu_index) else {
                continue;
            };
            if !is_owned_menu_item(&submenu_item) {
                submenu.removeItemAtIndex(submenu_index);
                removed += 1;
            }
        }
        return removed + remove_edge_and_duplicate_separators(&submenu);
    }

    0
}

pub fn remove_automatic_text_items<R: Runtime>(app_handle: &AppHandle<R>) {
    if let Err(error) = app_handle.run_on_main_thread(|| {
        let removed = remove_injected_items_from_application_menu();
        if removed > 0 {
            log::debug!("Removed {removed} macOS-injected application menu items");
        }
    }) {
        log::error!("Failed to schedule macOS application menu cleanup: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{DISABLED_AUTOMATIC_MENU_ITEMS, is_owned_menu_action};
    use objc2::sel;

    #[test]
    fn macos_bundle_declares_disabled_automatic_text_menu_items() {
        let info_plist = include_str!("../Info.plist");
        for key in DISABLED_AUTOMATIC_MENU_ITEMS {
            assert!(
                info_plist.contains(&format!("<key>{key}</key>\n  <true/>")),
                "{key} must be enabled in the macOS Info.plist"
            );
        }
    }

    #[test]
    fn keeps_only_sidecar_owned_menu_actions() {
        for action in [
            sel!(fireMenuItemAction:),
            sel!(cut:),
            sel!(copy:),
            sel!(paste:),
            sel!(selectAll:),
            sel!(terminate:),
        ] {
            assert!(is_owned_menu_action(action));
        }

        for injected_action in [
            sel!(submenuAction:),
            sel!(startDictation:),
            sel!(orderFrontCharacterPalette:),
        ] {
            assert!(!is_owned_menu_action(injected_action));
        }
    }
}
