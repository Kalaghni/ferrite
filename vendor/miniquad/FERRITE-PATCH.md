# FERRITE Mouse 4 / Mouse 5 patch

Based on miniquad 0.4.11 from crates.io. Upstream MIT and Apache-2.0
licenses are preserved. Registry metadata, lockfiles and debug output are
excluded from this source copy.

The only behavior change is support for two extra mouse buttons:

- Extra1 = Mouse 4 / Back; Extra2 = Mouse 5 / Forward. Existing enum values
  remain unchanged.
- Windows forwards WM_XBUTTONDOWN/UP using the high-word XBUTTON id and
  returns TRUE for handled messages.
- X11 forwards buttons 8/9 and leaves wheel buttons 4..7 untouched.
- Wayland maps BTN_SIDE/BTN_BACK and BTN_EXTRA/BTN_FORWARD.
- macOS reads NSEvent.buttonNumber for otherMouseDown/Up instead of mapping
  every extra button to Middle.
- The web bridge accepts extra buttons; the included gl.js prevents their
  default navigation while interacting with the canvas.

Native backends use shared translation helpers in src/event.rs. Their
unit tests run without a window, device, native desktop or additional
test dependencies:

    rustc --edition 2018 --test vendor/miniquad/src/event.rs -o target/miniquad-mouse-tests.exe
    target/miniquad-mouse-tests.exe

Macroquad's generic mouse state requires no patch. FERRITE's binding UI
handles capture, labels and config serialization separately. Cross-platform
translation tests do not replace a physical-device test on each platform.
