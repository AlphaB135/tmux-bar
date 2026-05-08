use cocoa::base::{id, nil, NO};
use cocoa::foundation::NSString;
use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};
use objc::runtime::{Object, Sel};
use std::ffi::CStr;
use std::process::Command;

static mut HANDLER: id = nil;

struct Session {
    name: String,
    command: String,
}

fn get_tmux_sessions() -> Vec<Session> {
    let names_out = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match names_out {
        Ok(out) => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .map(|name| {
                let cmd = Command::new("tmux")
                    .args([
                        "display-message",
                        "-t",
                        &name,
                        "-p",
                        "#{pane_current_command}",
                    ])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|| "?".into());
                Session { name, command: cmd }
            })
            .collect(),
        Err(_) => vec![],
    }
}

fn launch_iterm(session: &str) {
    let script = format!(
        r#"tell application "iTerm2"
activate
create window with default profile
tell current session of current window
    write text "tmux attach -t {session}"
end tell
end tell"#
    );
    let _ = Command::new("osascript").arg("-e").arg(script).spawn();
}

extern "C" fn on_open_session(_this: &Object, _sel: Sel, item: id) {
    unsafe {
        let obj: id = msg_send![item, representedObject];
        if obj != nil {
            let s: *const i8 = msg_send![obj, UTF8String];
            if !s.is_null() {
                launch_iterm(&CStr::from_ptr(s).to_string_lossy());
            }
        }
    }
}

extern "C" fn on_menu_open(_this: &Object, _sel: Sel, menu: id) {
    unsafe {
        let () = msg_send![menu, removeAllItems];

        let sessions = get_tmux_sessions();

        if sessions.is_empty() {
            let t = NSString::alloc(nil).init_str("No tmux sessions");
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item,
                initWithTitle:t
                action:nil
                keyEquivalent:NSString::alloc(nil).init_str("")
            ];
            let () = msg_send![item, setEnabled: NO];
            let () = msg_send![menu, addItem: item];
        } else {
            let handler = HANDLER;
            for s in &sessions {
                let label = format!("{}  \u{2014}  {}", s.name, s.command);
                let t = NSString::alloc(nil).init_str(&label);
                let obj = NSString::alloc(nil).init_str(&s.name);
                let item: id = msg_send![class!(NSMenuItem), alloc];
                let item: id = msg_send![item,
                    initWithTitle:t
                    action:sel!(openSession:)
                    keyEquivalent:NSString::alloc(nil).init_str("")
                ];
                let () = msg_send![item, setRepresentedObject: obj];
                let () = msg_send![item, setTarget: handler];
                let () = msg_send![menu, addItem: item];
            }
        }

        let sep: id = msg_send![class!(NSMenuItem), separatorItem];
        let () = msg_send![menu, addItem: sep];

        let qt = NSString::alloc(nil).init_str("Quit");
        let qi: id = msg_send![class!(NSMenuItem), alloc];
        let qi: id = msg_send![qi,
            initWithTitle:qt
            action:sel!(terminate:)
            keyEquivalent:NSString::alloc(nil).init_str("q")
        ];
        let () = msg_send![menu, addItem: qi];
    }
}

fn main() {
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let () = msg_send![app, setActivationPolicy: 1]; // Accessory = no dock icon

        let mut hd = ClassDecl::new("TmuxHandler", class!(NSObject)).unwrap();
        hd.add_method(
            sel!(openSession:),
            on_open_session as extern "C" fn(&Object, Sel, id),
        );
        let hc = hd.register();
        HANDLER = msg_send![hc, new];

        let mut dd = ClassDecl::new("MenuDelegate", class!(NSObject)).unwrap();
        dd.add_method(
            sel!(menuWillOpen:),
            on_menu_open as extern "C" fn(&Object, Sel, id),
        );
        let dc = dd.register();
        let delegate: id = msg_send![dc, new];

        let sb: id = msg_send![class!(NSStatusBar), systemStatusBar];
        let si: id = msg_send![sb, statusItemWithLength: -1.0];
        let btn: id = msg_send![si, button];
        let () = msg_send![btn, setTitle: NSString::alloc(nil).init_str("TM")];

        let menu: id = msg_send![class!(NSMenu), new];
        let () = msg_send![menu, setDelegate: delegate];
        let () = msg_send![si, setMenu: menu];

        let () = msg_send![app, run];
    }
}
