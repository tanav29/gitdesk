use super::Desk;
use crate::view::{ACCENT, INK, MUTED, RULE, button, color, label};
use rune::{Color, El, Family, Key, TextStyle, col, el, row, text};
use rune_kit::{ButtonVariant, Kit};

const CELL: f32 = 8.0;
const LINE: f32 = 18.0;
const INSET: f32 = 8.0;
const BACKGROUND: u32 = 0xf8fafd;
const FOREGROUND: u32 = 0x172437;

fn rgb(value: u32) -> Color {
    let channel = |shift| rune::color::srgb_decode(((value >> shift) & 255u32) as f32 / 255.0);
    Color {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: 1.0,
    }
}
fn palette(color: vt100::Color, fallback: u32) -> Color {
    let n = match color {
        vt100::Color::Default => fallback,
        vt100::Color::Rgb(r, g, b) => u32::from_be_bytes([0, r, g, b]),
        vt100::Color::Idx(i @ 0..=15) => [
            0x172437, 0x9e2932, 0x1e6240, 0x78530c, 0x2448a6, 0x793f8c, 0x16606d, 0x4e5a6d,
            0x5e6b7e, 0xb13d41, 0x2a754c, 0x86601b, 0x315ab8, 0x884da2, 0x19717e, 0x59667a,
        ][i as usize],
        vt100::Color::Idx(i @ 16..=231) => {
            let cube = [0, 95, 135, 175, 215, 255];
            let i = usize::from(i - 16);
            u32::from_be_bytes([0, cube[i / 36], cube[(i / 6) % 6], cube[i % 6]])
        }
        vt100::Color::Idx(i) => (8 + u32::from(i - 232) * 10) * 0x010101,
    };
    rgb(n)
}

fn grid(screen: &vt100::Screen, mut panel: El) -> El {
    let (rows, cols) = screen.size();
    for y in 0..rows.get() {
        let mut x = 0;
        while x < cols.get() {
            let start = x;
            let first = screen.cell(y, x).unwrap();
            let style = |c: &vt100::Cell| {
                (
                    c.fgcolor(),
                    c.bgcolor(),
                    c.inverse(),
                    c.bold(),
                    c.italic(),
                    c.underline(),
                )
            };
            let mut content = String::new();
            while x < cols.get() {
                let c = screen.cell(y, x).unwrap();
                if style(c) != style(first) {
                    break;
                }
                if !c.is_wide_continuation() {
                    content.push_str(if c.has_contents() { c.contents() } else { " " });
                }
                x += 1;
            }
            let (mut fg, mut bg) = (
                palette(first.fgcolor(), FOREGROUND),
                palette(first.bgcolor(), BACKGROUND),
            );
            if first.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            let left = INSET + CELL * f32::from(start);
            let top = INSET + LINE * f32::from(y);
            let width = CELL * f32::from(x - start);
            if first.inverse() || first.bgcolor() != vt100::Color::Default {
                panel = panel.child(el().absolute(left, top).size(width, LINE).fill(bg));
            }
            if !content.trim_end().is_empty() {
                let mut font = TextStyle::new(13.0, if first.bold() { 600.0 } else { 400.0 })
                    .family(Family::SystemMono)
                    .lh(LINE);
                if first.italic() {
                    font = font.italic();
                }
                panel = panel.child(
                    text(content.trim_end().to_owned())
                        .absolute(left, top)
                        .font(font)
                        .color(fg),
                );
            }
            if first.underline() {
                panel = panel.child(
                    el().absolute(left, top + LINE - 2.0)
                        .size(width, 1.0)
                        .fill(fg),
                );
            }
        }
    }
    if !screen.hide_cursor() && screen.scrollback() == 0 {
        let (y, x) = screen.cursor_position();
        panel = panel.child(
            el().absolute(
                INSET + CELL * f32::from(x),
                INSET + LINE * f32::from(y) + LINE - 2.0,
            )
            .size(CELL, 2.0)
            .fill(rgb(FOREGROUND)),
        );
    }
    panel
}

fn paste_from_clipboard(s: &mut Desk, tx: &mut rune::Tx) {
    let repo = s.workspace.selected.clone();
    let tab = s.tab;
    tx.read_clipboard(move |s: &mut Desk, clip, _| {
        if repo == s.workspace.selected
            && tab == s.tab
            && let Some(t) = s.shell()
        {
            t.paste(clip.text());
        }
    });
}

pub fn view(s: &Desk, k: &Kit) -> El {
    let Some(shell) = s.shell() else {
        return row().child(
            button("Start terminal")
                .variant(ButtonVariant::Contrast)
                .disabled(s.busy)
                .on_press(|s: &mut Desk| s.start_shell())
                .build(k),
        );
    };
    let title = if shell.title == "Terminal" {
        el()
    } else {
        label(&shell.title, 14.0, 600.0, INK).min_w(0.0).truncate()
    };
    let mut bar = row()
        .gap(8.0)
        .center_y()
        .child(title.grow(1.0))
        .child(
            button("Paste")
                .disabled(shell.ended.is_some())
                .on_press_tx(|s: &mut Desk, tx| {
                    tx.focus("terminal-screen", false);
                    paste_from_clipboard(s, tx);
                })
                .build(k),
        )
        .child(
            button("Copy screen")
                .on_press_tx(|s: &mut Desk, tx| {
                    if let Some(shell) = s.shell() {
                        tx.copy(shell.screen(|screen| screen.contents()));
                    }
                })
                .build(k),
        );
    if let Some(status) = &shell.ended {
        bar = bar.child(label(status, 13.0, 400.0, MUTED)).child(
            button("Restart")
                .on_press(|s: &mut Desk| s.start_shell())
                .build(k),
        );
    }
    let panel = el()
        .key("terminal-screen")
        .role("textbox")
        .label("Repository terminal")
        .focusable(true)
        .autofocus()
        .grow(1.0)
        .w_full()
        .min_h(0.0)
        .clip_rounded(6.0)
        .fill(rgb(BACKGROUND))
        .border(1.0, color(RULE))
        .focus_ring(2.0, 0.0, color(ACCENT))
        .on_layout(|s: &mut Desk, [_, _, w, h]| {
            if let Some(t) = s.shell() {
                t.resize(
                    ((h - 2.0 * INSET) / LINE).clamp(2.0, 250.0) as u16,
                    ((w - 2.0 * INSET) / CELL).clamp(8.0, 500.0) as u16,
                );
            }
        })
        .on_press_tx(|_: &mut Desk, tx| tx.focus("terminal-screen", false))
        .on_key(|s: &mut Desk, ev, tx| {
            if ev.mods.ctrl && ev.key == Key::Function(6) {
                tx.focus("tab-Terminal", true);
                return true;
            }
            if (ev.mods.logo || ev.mods.shift) && ev.key == Key::Command('v') {
                paste_from_clipboard(s, tx);
                true
            } else {
                s.shell().is_some_and(|t| t.key(ev))
            }
        });
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(6.0)
        .child(bar)
        .child(shell.screen(|screen| grid(screen, panel)))
}
