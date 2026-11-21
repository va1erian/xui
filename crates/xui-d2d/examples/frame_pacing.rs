//! Measures how fast a Direct2D window can really present on each monitor.
//!
//! For every monitor it opens a small borderless window there (never
//! activated), repaints it for a moment under each combination of present
//! option and frame tick, and prints the frame intervals it got:
//!
//! - present `vsync` is what `xui-d2d` uses today
//!   (`D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS`); `immediate` adds
//!   `D2D1_PRESENT_OPTIONS_IMMEDIATELY`.
//! - tick `free-run` repaints as fast as the present allows, which shows
//!   whether and at what rate `EndDraw` blocks; `high-res` waits on a
//!   high-resolution waitable timer at the monitor's refresh rate, the
//!   candidate frame tick; `SetTimer` is a 1 ms `WM_TIMER`, today's tick.
//!
//! It talks to Win32 directly instead of going through the backend, because
//! it measures the system, not xui. It exits by itself after about nine
//! seconds per monitor. Run it once per primary-monitor arrangement:
//!
//! ```text
//! cargo run -p xui-d2d --release --example frame_pacing
//! ```

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    if let Err(error) = probe::run() {
        eprintln!("frame_pacing: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod probe {
    #![allow(unsafe_code)]

    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
        D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS, D2D1_PRESENT_OPTIONS_IMMEDIATELY,
        D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS, D2D1_RENDER_TARGET_PROPERTIES,
        D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory,
        ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Gdi::{
        DEVMODEW, ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_FLAGS, EnumDisplayMonitors,
        EnumDisplaySettingsExW, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE, SetWaitableTimer,
        TIMER_ALL_ACCESS,
    };
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
        MONITORINFOF_PRIMARY, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, RegisterClassExW, SW_SHOWNOACTIVATE, SetTimer, ShowWindow,
        TranslateMessage, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use windows::core::{BOOL, PCWSTR, Result, w};

    /// How long each combination repaints.
    const RUN: Duration = Duration::from_millis(1500);
    /// Frames ignored at the start of a run, while the target settles.
    const WARMUP: usize = 20;
    /// The window's side, in device pixels.
    const SIZE: i32 = 480;
    const CLASS: PCWSTR = w!("xui-frame-pacing");

    #[derive(Clone, Copy)]
    enum Tick {
        FreeRun,
        HighRes,
        SetTimer,
    }

    struct Monitor {
        name: String,
        area: RECT,
        hz: u32,
        primary: bool,
    }

    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the `&mut Vec<HMONITOR>` that `monitors` passes and
        // keeps alive for the whole (synchronous) enumeration.
        let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
        list.push(monitor);
        BOOL::from(true)
    }

    fn monitors() -> Vec<Monitor> {
        let mut handles: Vec<HMONITOR> = Vec::new();
        // SAFETY: `collect` only runs during this call, with a pointer to
        // `handles`, which outlives it.
        let _ = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(collect),
                LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
            )
        };
        handles.into_iter().filter_map(describe).collect()
    }

    fn describe(monitor: HMONITOR) -> Option<Monitor> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: `cbSize` announces the extended struct, whose first field is
        // the `MONITORINFO` the function writes through.
        let found = unsafe {
            GetMonitorInfoW(
                monitor,
                &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
            )
        };
        if !found.as_bool() {
            return None;
        }
        let mut mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: `szDevice` is the NUL-terminated name just filled in, and
        // `mode` is a writable `DEVMODEW` with its size set.
        let known = unsafe {
            EnumDisplaySettingsExW(
                PCWSTR(info.szDevice.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            )
        };
        let length = info.szDevice.iter().take_while(|&&unit| unit != 0).count();
        Some(Monitor {
            name: String::from_utf16_lossy(&info.szDevice[..length]),
            area: info.monitorInfo.rcWork,
            // 0 and 1 mean "the hardware default", which says nothing.
            hz: if known.as_bool() && mode.dmDisplayFrequency > 1 {
                mode.dmDisplayFrequency
            } else {
                60
            },
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        })
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwards the arguments the system gave this procedure.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn open(area: RECT) -> Result<HWND> {
        let x = area.left + (area.right - area.left - SIZE) / 2;
        let y = area.top + (area.bottom - area.top - SIZE) / 2;
        // SAFETY: a plain popup of the class registered in `run`; it is shown
        // without being activated, so it takes no focus from the desktop.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                CLASS,
                w!("xui frame pacing"),
                WS_POPUP,
                x,
                y,
                SIZE,
                SIZE,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(hwnd)
        }
    }

    fn target(
        factory: &ID2D1Factory,
        hwnd: HWND,
        present: D2D1_PRESENT_OPTIONS,
    ) -> Result<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush)> {
        // The same properties as `sys/gfx/target.rs`, but for `presentOptions`.
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let window = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: SIZE as u32,
                height: SIZE as u32,
            },
            presentOptions: present,
        };
        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        // SAFETY: both property structs and the colour are valid for the calls.
        unsafe {
            let rt = factory.CreateHwndRenderTarget(&properties, &window)?;
            let brush = rt.CreateSolidColorBrush(&white, None)?;
            Ok((rt, brush))
        }
    }

    /// Dispatches every message already queued.
    fn pump() {
        let mut msg = MSG::default();
        // SAFETY: the standard message calls on the thread that owns the window.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// Repaints `hwnd` for [`RUN`] and returns when each frame was presented.
    fn measure(
        factory: &ID2D1Factory,
        hwnd: HWND,
        present: D2D1_PRESENT_OPTIONS,
        tick: Tick,
        hz: u32,
    ) -> Result<Vec<Instant>> {
        let (rt, brush) = target(factory, hwnd, present)?;
        let mut stamps: Vec<Instant> = Vec::with_capacity(8192);
        let frame = |stamps: &mut Vec<Instant>| {
            let x = (stamps.len() as i32 * 4 % SIZE) as f32;
            let bar = D2D_RECT_F {
                left: x,
                top: 0.0,
                right: x + 24.0,
                bottom: SIZE as f32,
            };
            let black = D2D1_COLOR_F {
                a: 1.0,
                ..Default::default()
            };
            // SAFETY: one frame on a live target: a clear and a rectangle with
            // a brush that target owns.
            unsafe {
                rt.BeginDraw();
                rt.Clear(Some(&black));
                rt.FillRectangle(&bar, &brush);
                let _ = rt.EndDraw(None, None);
            }
            stamps.push(Instant::now());
        };
        let start = Instant::now();
        match tick {
            Tick::FreeRun => {
                while start.elapsed() < RUN {
                    pump();
                    frame(&mut stamps);
                }
            }
            Tick::SetTimer => {
                let mut msg = MSG::default();
                // SAFETY: a timer on this thread's window, read back from its
                // own queue and killed before returning.
                unsafe {
                    SetTimer(Some(hwnd), 1, 1, None);
                    while start.elapsed() < RUN && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        if msg.message == WM_TIMER {
                            frame(&mut stamps);
                        } else {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                    let _ = KillTimer(Some(hwnd), 1);
                }
            }
            Tick::HighRes => {
                // SAFETY: an unnamed timer, closed below.
                let timer = unsafe {
                    CreateWaitableTimerExW(
                        None,
                        PCWSTR::null(),
                        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                        TIMER_ALL_ACCESS.0,
                    )
                }?;
                let period = Duration::from_secs_f64(1.0 / f64::from(hz));
                let mut due = Instant::now() + period;
                while start.elapsed() < RUN {
                    if wait(timer, due)? {
                        frame(&mut stamps);
                        // Deadlines advance by the period so a late frame does
                        // not delay the next, unless a whole period was lost.
                        due = (due + period).max(Instant::now());
                    } else {
                        pump();
                    }
                }
                // SAFETY: the handle created above, not used afterwards.
                let _ = unsafe { CloseHandle(timer) };
            }
        }
        Ok(stamps)
    }

    /// Waits until `due` or until a message arrives; `true` when `due` came.
    fn wait(timer: HANDLE, due: Instant) -> Result<bool> {
        let remaining = due.saturating_duration_since(Instant::now());
        // A negative due time is relative, in 100 ns units; zero would mean
        // an absolute time in 1601.
        let relative = -((remaining.as_nanos() / 100) as i64).max(1);
        // SAFETY: `timer` is a live waitable timer and `relative` outlives the
        // call; the wait only reads the one-element handle array.
        unsafe {
            SetWaitableTimer(timer, &relative, 0, None, None, false)?;
            let woke = MsgWaitForMultipleObjectsEx(
                Some(&[timer]),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            Ok(woke == WAIT_OBJECT_0)
        }
    }

    fn report(present: &str, tick: &str, stamps: &[Instant]) {
        let mut gaps: Vec<f64> = stamps
            .get(WARMUP..)
            .unwrap_or_default()
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
            .collect();
        if gaps.is_empty() {
            println!("  {present:<10} {tick:<9} no frames");
            return;
        }
        gaps.sort_by(f64::total_cmp);
        let median = gaps[gaps.len() / 2];
        let p99 = gaps[(gaps.len() * 99 / 100).min(gaps.len() - 1)];
        let worst = gaps[gaps.len() - 1];
        let late = gaps.iter().filter(|&&gap| gap > median * 1.5).count();
        println!(
            "  {present:<10} {tick:<9} {:>8.1} {median:>9.3} {p99:>8.3} {worst:>8.3} {late:>5}/{}",
            1000.0 / median,
            gaps.len(),
        );
    }

    pub fn run() -> Result<()> {
        // SAFETY: process-wide setup before any window exists; the class has a
        // static name and procedure, and the factory is used on this thread.
        let factory: ID2D1Factory = unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: GetModuleHandleW(None)?.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassExW(&class);
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?
        };
        let presents = [
            ("vsync", D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS),
            (
                "immediate",
                D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS | D2D1_PRESENT_OPTIONS_IMMEDIATELY,
            ),
        ];
        let ticks = [
            ("free-run", Tick::FreeRun),
            ("high-res", Tick::HighRes),
            ("SetTimer", Tick::SetTimer),
        ];
        for monitor in monitors() {
            println!(
                "{}{}: {} Hz reported, period {:.3} ms",
                monitor.name,
                if monitor.primary { " (primary)" } else { "" },
                monitor.hz,
                1000.0 / f64::from(monitor.hz),
            );
            println!("  present    tick           fps    median      p99    worst  late/frames");
            let hwnd = open(monitor.area)?;
            for (present_name, present) in presents {
                for (tick_name, tick) in ticks {
                    let stamps = measure(&factory, hwnd, present, tick, monitor.hz)?;
                    report(present_name, tick_name, &stamps);
                }
            }
            // SAFETY: the window opened above; its target is already dropped.
            let _ = unsafe { DestroyWindow(hwnd) };
            pump();
        }
        Ok(())
    }
}
