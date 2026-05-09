use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};
use objc::runtime::{Object, Sel};
use std::ffi::CStr;
use std::process::Command;

#[link(name = "WebKit", kind = "framework")]
extern "C" {}

static mut HANDLER: id = nil;
static mut WEB_HANDLER: id = nil;
static mut POPOVER: id = nil;
static mut STATUS_ITEM: id = nil;
static mut WEBVIEW: id = nil;
static mut TIMER: id = nil;

struct Session {
    name: String,
    command: String,
}

fn get_tmux_sessions() -> Vec<Session> {
    let out = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
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
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .trim()
                            .to_string()
                    })
                    .unwrap_or_else(|| "?".into());
                Session { name, command: cmd }
            })
            .collect(),
        Err(_) => vec![],
    }
}

fn capture_pane(session: &str) -> String {
    Command::new("tmux")
        .args(["capture-pane", "-e", "-t", session, "-p", "-S", "-30"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim_end()
                .to_string()
        })
        .unwrap_or_else(|| "Failed to capture".to_string())
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn generate_html() -> String {
    let sessions = get_tmux_sessions();
    let mut cards = String::new();

    if sessions.is_empty() {
        cards.push_str(r#"<div class="empty">No tmux sessions</div>"#);
    } else {
        for s in &sessions {
            let raw_output = capture_pane(&s.name);
            let colored = ansi_to_html::convert(&raw_output)
                .unwrap_or_else(|_| html_escape(&raw_output));
            let name = html_escape(&s.name);
            let cmd = html_escape(&s.command);
            let sid = s.name.replace(' ', "-").replace('.', "-");
            cards.push_str(&format!(
                r##"<div class="card" onclick="action('open','{n}')">
  <div class="card-hd">
    <div><span class="name">{n}</span><span class="cmd">{c}</span></div>
    <button class="ibtn" onclick="event.stopPropagation();action('open','{n}')">iTerm2</button>
  </div>
  <pre class="out" id="out-{i}">{o}</pre>
</div>"##,
                n = name,
                c = cmd,
                i = sid,
                o = colored,
            ));
        }
    }

    format!(
        r##"<!DOCTYPE html>
<html><head><meta charset="UTF-8">
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{background:#1a1a2e;color:#e0e0e0;font-family:-apple-system,BlinkMacSystemFont,'Sukhumvit Set',sans-serif;padding:10px}}
.hdr{{display:flex;justify-content:space-between;align-items:center;padding:4px 4px 10px}}
.hdr span{{font-size:.8rem;font-weight:600;color:#94a3b8}}
.hdr .dot{{width:6px;height:6px;border-radius:50%;background:#22c55e;display:inline-block;margin-right:6px;box-shadow:0 0 4px #22c55e80}}
.qbtn{{background:#334155;color:#94a3b8;border:none;border-radius:4px;padding:3px 12px;font-size:.65rem;cursor:pointer}}
.qbtn:hover{{background:#ef4444;color:#fff}}
.grid{{display:flex;flex-direction:column;gap:8px}}
.card{{background:#16213e;border-radius:8px;border:1px solid #0f3460;overflow:hidden;cursor:pointer;transition:border-color .15s}}
.card:hover{{border-color:#533483}}
.card-hd{{padding:8px 12px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #0f3460}}
.name{{font-weight:700;font-size:.85rem;color:#e94560}}
.cmd{{font-size:.65rem;color:#64748b;margin-left:8px;font-family:Menlo,monospace}}
.ibtn{{background:#533483;color:#fff;border:none;border-radius:4px;padding:3px 10px;font-size:.65rem;cursor:pointer}}
.ibtn:hover{{background:#e94560}}
.out{{padding:8px 12px;background:#08080c;font-family:Menlo,monospace;font-size:.7rem;color:#cdd6f4;max-height:300px;overflow-y:auto;overflow-x:hidden;white-space:pre-wrap;word-break:break-all;line-height:1.5}}
.empty{{text-align:center;padding:30px;color:#64748b}}
</style>
</head><body>
<div class="hdr"><span><span class="dot"></span>TMux Sessions</span><button class="qbtn" onclick="action('quit','')">Quit</button></div>
<div class="grid">{cards}</div>
<script>
function action(t,n){{window.webkit.messageHandlers.bridge.postMessage(t+':'+n)}}
</script>
</body></html>"##,
        cards = cards,
    )
}

fn json_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn generate_refresh_script() -> String {
    let sessions = get_tmux_sessions();
    let mut js = String::from("(function(){");
    for s in &sessions {
        let raw_output = capture_pane(&s.name);
        let colored = ansi_to_html::convert(&raw_output)
            .unwrap_or_else(|_| html_escape(&raw_output));
        let sid = s.name.replace(' ', "-").replace('.', "-");
        let json_html = json_encode(&colored);
        js.push_str(&format!(
            "var e=document.getElementById('out-{}');if(e)e.innerHTML={};",
            sid, json_html,
        ));
    }
    js.push_str("})();");
    js
}

unsafe fn build_webview() -> id {
    let frame = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: 520.0,
            height: 480.0,
        },
    };

    let config: id = msg_send![class!(WKWebViewConfiguration), alloc];
    let config: id = msg_send![config, init];

    let uc: id = msg_send![config, userContentController];
    let () = msg_send![uc,
        addScriptMessageHandler: WEB_HANDLER
        name: NSString::alloc(nil).init_str("bridge")
    ];

    let wv: id = msg_send![class!(WKWebView), alloc];
    let () = msg_send![wv, initWithFrame: frame configuration: config];
    let () = msg_send![wv, setOpaque: NO];

    let html = generate_html();
    let ns = NSString::alloc(nil).init_str(&html);
    let () = msg_send![wv, loadHTMLString: ns baseURL: nil];

    wv
}

extern "C" fn on_toggle(_this: &Object, _sel: Sel, _sender: id) {
    unsafe {
        let showing: bool = msg_send![POPOVER, isShown];
        if showing {
            if TIMER != nil {
                let () = msg_send![TIMER, invalidate];
                TIMER = nil;
            }
            let () = msg_send![POPOVER, performClose: nil];
        } else {
            let wv = build_webview();
            WEBVIEW = wv;

            let vc: id = msg_send![class!(NSViewController), alloc];
            let () = msg_send![vc, init];
            let () = msg_send![vc, setView: wv];
            let () = msg_send![POPOVER,
                setContentSize: NSSize {
                    width: 520.0,
                    height: 480.0,
                }
            ];
            let () = msg_send![POPOVER, setContentViewController: vc];

            let btn: id = msg_send![STATUS_ITEM, button];
            let bounds: NSRect = msg_send![btn, bounds];
            let () = msg_send![POPOVER,
                showRelativeToRect: bounds
                ofView: btn
                preferredEdge: 1u64
            ];

            TIMER = msg_send![class!(NSTimer),
                scheduledTimerWithTimeInterval: 3.0
                target: HANDLER
                selector: sel!(refreshWeb:)
                userInfo: nil
                repeats: YES
            ];
        }
    }
}

extern "C" fn on_refresh(_this: &Object, _sel: Sel, _timer: id) {
    unsafe {
        let showing: bool = msg_send![POPOVER, isShown];
        if !showing {
            if TIMER != nil {
                let () = msg_send![TIMER, invalidate];
                TIMER = nil;
            }
            return;
        }
        if WEBVIEW != nil {
            let script = generate_refresh_script();
            let ns = NSString::alloc(nil).init_str(&script);
            let () = msg_send![WEBVIEW, evaluateJavaScript: ns completionHandler: nil];
        }
    }
}

extern "C" fn on_popover_closed(
    _this: &Object,
    _sel: Sel,
    _notif: id,
) {
    unsafe {
        if TIMER != nil {
            let () = msg_send![TIMER, invalidate];
            TIMER = nil;
        }
    }
}

extern "C" fn on_web_msg(
    _this: &Object,
    _sel: Sel,
    _ctrl: id,
    msg: id,
) {
    unsafe {
        let body: id = msg_send![msg, body];
        let s: *const i8 = msg_send![body, UTF8String];
        if !s.is_null() {
            let m = CStr::from_ptr(s).to_string_lossy();
            if let Some(name) = m.strip_prefix("open:") {
                launch_iterm(name);
            } else if m.starts_with("quit:") {
                let app: id =
                    msg_send![class!(NSApplication), sharedApplication];
                let () = msg_send![app, terminate: nil];
            }
        }
    }
}

fn main() {
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let () = msg_send![app, setActivationPolicy: 1];

        let mut hd =
            ClassDecl::new("TmuxHandler", class!(NSObject)).unwrap();
        hd.add_method(
            sel!(togglePopover:),
            on_toggle as extern "C" fn(&Object, Sel, id),
        );
        hd.add_method(
            sel!(refreshWeb:),
            on_refresh as extern "C" fn(&Object, Sel, id),
        );
        hd.add_method(
            sel!(popoverDidClose:),
            on_popover_closed as extern "C" fn(&Object, Sel, id),
        );
        let hc = hd.register();
        HANDLER = msg_send![hc, new];

        let mut wh =
            ClassDecl::new("WebHandler", class!(NSObject)).unwrap();
        wh.add_method(
            sel!(userContentController:didReceiveScriptMessage:),
            on_web_msg as extern "C" fn(&Object, Sel, id, id),
        );
        let wc = wh.register();
        WEB_HANDLER = msg_send![wc, new];

        let sb: id = msg_send![class!(NSStatusBar), systemStatusBar];
        STATUS_ITEM = msg_send![sb, statusItemWithLength: -1.0];
        let btn: id = msg_send![STATUS_ITEM, button];
        let () = msg_send![btn,
            setTitle: NSString::alloc(nil).init_str("TM")
        ];
        let () = msg_send![btn, setTarget: HANDLER];
        let () = msg_send![btn, setAction: sel!(togglePopover:)];

        POPOVER = msg_send![class!(NSPopover), alloc];
        let () = msg_send![POPOVER, init];
        let () = msg_send![POPOVER, setBehavior: 2u64];
        let () = msg_send![POPOVER, setDelegate: HANDLER];

        let () = msg_send![app, run];
    }
}
